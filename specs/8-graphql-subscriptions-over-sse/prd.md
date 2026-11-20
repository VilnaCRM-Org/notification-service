---
stepsCompleted: [autonomous-create-prd]
inputDocuments:
  - https://github.com/VilnaCRM-Org/notification-service/issues/8
  - specs/8-graphql-subscriptions-over-sse/research.md
  - specs/8-graphql-subscriptions-over-sse/product-brief.md
  - specs/4-realtime-event-delivery-hub/prd.md
date: 2026-11-16
author: bmad-autonomous-planning (create-prd stage)
---

# PRD: GraphQL subscriptions over Server-Sent Events (issue #8)

Source: <https://github.com/VilnaCRM-Org/notification-service/issues/8>, as read on 2026-11-16 and again on 2026-11-17, after the maintainer's decision of 2026-11-17 added two criteria (every route of the service in the OpenAPI document and on the Swagger page; the OpenAPI gates and Schemathesis green). Bundle: `specs/8-graphql-subscriptions-over-sse/`. Context: `notification/event-stream` (application layer) and the composition root `crates/service`. Inputs: `research.md` (its section 8 holds the resolved questions), `product-brief.md` (success criteria S1 to S15) and the maintainer decisions M1 to M3 named there. Decision M4 of 2026-11-20 (answer "Ignore both, replace ours"): the RustSec advisories RUSTSEC-2026-0542 (humantime) and RUSTSEC-2026-0543 (chrono), published after 2026-11-17, made `make check-security` fail; both are ignored in the protected `deny.toml` by two approved entries with a reason and a removal condition, and humantime is replaced in the two service crates in this pull request (NFR-21, NFR-15, `research.md` section 9).

Notation:

- `FR-<n>` and `NFR-<n>` are the requirements of this PRD. The numbers are new and local to this bundle.
- `4/FR-<n>` and `4/NFR-<n>` are the requirements of `specs/4-realtime-event-delivery-hub/prd.md`. They stay in force and this PRD reuses them by reference.
- `AC1` to `AC11` are the acceptance criteria of issue #8, in the order the issue lists them on 2026-11-17. AC9 (every route in the OpenAPI document and on the Swagger page) and AC10 (the OpenAPI gates and Schemathesis) are the two criteria of the maintainer's decision of 2026-11-17; the former last criterion (documents and `make ci`) is now AC11.

## Executive Summary

Frontends query the service through GraphQL (`POST /api/graphql`: `eventTypes`, `eventStreamStatus`), but they cannot receive its events through GraphQL. The schema has no subscription root, so a GraphQL client has to open a second, non-GraphQL connection to `GET /api/events/stream` or `GET /api/events/ws`, with its own frame format, errors and reconnect handling. This change makes the event stream a GraphQL subscription served over Server-Sent Events. It follows the GraphQL over SSE protocol of the graphql-sse project in its distinct connections mode, and the GraphQL over HTTP specification for the request formats (M1).

What a client does:

1. It sends `subscription { events(types: ["user.created"]) { id type data } }` to `/api/graphql` with `Accept: text/event-stream`: as a POST with a JSON body (the graphql-sse client, urql, Relay, Apollo through a link), or as a GET with the operation in the query string (a browser `EventSource`).
2. It receives `200 text/event-stream`: a comment, then one `event: next` per event of a listed type, carrying `id: <event id>` and `data: {"data":{"events":{...}}}`, and a comment after every `STREAM_KEEP_ALIVE` of silence.
3. After a lost connection it reconnects to any node with the last id, in the `Last-Event-ID` header or the `lastEventId` argument, and receives every retained event after that id, in order, before live events.

How it works:

- The GraphQL subscription is one more subscriber of the node's existing hub. It has the window, the replay from Kafka, the cursor rules, the type rules, the subscriber cap, the readiness rule, the shutdown and the slow-client handling of the SSE and WebSocket routes of bundle 4. Delivery stays at least once, in order and resumable on any node, and the client deduplicates by the envelope id.
- The transport is chosen by the `Accept` header on the existing path. A request that does not ask for an event stream keeps today's JSON answer.
- An invalid operation is answered inside an accepted event stream, with `VALIDATION_FAILED` or the GraphQL document errors, then `complete`, as the protocol requires. A node that cannot take the subscription answers the 503 problem of the REST streaming routes before any stream, and graphql-sse clients retry it.
- Every route the service serves becomes an operation of the OpenAPI document and of the Swagger page at `/api/docs`, fully documented: the two streaming routes, both forms of the GraphQL endpoint, the liveness probe and the documentation routes join the three REST operations, and a test fails when a route is served without an entry (FR-26 to FR-29, maintainer decision of 2026-11-17).
- The change adds no configuration variable, no bounded context and no state shared between requests. Its one dependency change replaces `humantime` with `jiff` in `crates/service` and drops it from the event-stream application crate (NFR-21, M4). Its protected changes are the four the maintainer approved, committed outside the stories (NFR-15).

### Protocol choices

Each choice is specified as a testable requirement in the FR named in the last column. Its reason is the `> Assumption:` line under that FR.

| # | Choice | Decision | FR |
| --- | --- | --- | --- |
| P1 | Protocol and mode | the graphql-sse protocol, distinct connections mode only; no single connection mode, no GraphQL WebSocket protocol | FR-2, FR-5 |
| P2 | Endpoint | `/api/graphql` itself, chosen by content negotiation; no sibling path | FR-2 |
| P3 | Negotiation | `Accept` read as a list of media ranges with q values. POST: an event stream when `text/event-stream` outranks the two JSON types; ties and wildcards stay on JSON. GET: the event stream is the only representation besides the explorer, so a wildcard range admits it, a missing header reads as `application/json`, and `text/html` or a JSON type ranked as high keeps today's answer | FR-2 |
| P4 | Request formats | POST with a JSON body (GraphQL over HTTP 4.4); GET with `query`, `operationName`, `variables` and `extensions` in the query string (GraphQL over HTTP 4.3), in every `APP_ENV` | FR-3, FR-4 |
| P5 | Field and object | `events(types: [String!]!, lastEventId: String): Event!` and `Event { id: String!, type: String!, data: JSON! }`, the object of a WebSocket frame | FR-1, FR-10 |
| P6 | Events on the wire | `next` per execution result; `complete` with a bare `data:` line after a single result; the comment `:` as keep-alive; nothing else | FR-8 |
| P7 | Event ids | `id: <event id>` on every subscription result, none on a single result; no `retry:` field | FR-8 |
| P8 | Resume | the `Last-Event-ID` header wins over the `lastEventId` argument; the window and replay of bundle 4 | FR-11 |
| P9 | Errors before the stream | only transport errors (415, 413, 414, 400, 422 problems), the 503 states and the request budget | FR-6, FR-16 |
| P10 | Errors in the stream | document errors, the direct-field rule (exactly one field selection written directly at the root) and invalid arguments: one `next` carrying the errors, then `complete`; the last two with `VALIDATION_FAILED` | FR-13, FR-14, FR-15 |
| P11 | 503 states | starting, at capacity, shutting down: the 503 problem of the REST streaming routes instead of a head | FR-16 |
| P12 | When the head is written | after the operation registered its subscriber, was refused or produced its first result, within `HTTP_REQUEST_TIMEOUT` | FR-9 |
| P13 | Keep-alive | a comment right after the head, then after every `STREAM_KEEP_ALIVE` without a write | FR-17 |
| P14 | Slow clients | the hub's fall-behind and catch-up; the socket send bound of every connection | FR-18 |
| P15 | End and shutdown | `complete` only after a single result; a subscription cut by shutdown or a failed replay ends without it, so the client resumes | FR-19 |
| P16 | Queries and mutations over SSE | a query: one `next`, then `complete`; a mutation: a document error, because the schema has no mutation root | FR-20 |
| P17 | CORS | the existing allow-list, unchanged, proved for both forms | FR-21 |
| P18 | Limits | body and query string `HTTP_MAX_BODY_BYTES`, depth 16, complexity 1 000 with `Event.data` costing 501 (so a document selects it at most once), one root field, 100 types, a 64-bit cursor, one operation per request, `MAX_SUBSCRIBERS` | FR-1, FR-4, NFR-8 |
| P19 | Response head | `text/event-stream` without a charset, `cache-control: no-cache`, `x-accel-buffering: no`, `vary: accept`; no `connection` header | FR-7 |
| P20 | Caching of negotiated answers | `Vary: Accept` on every event stream and on every answer to a GET of `/api/graphql` | FR-5, FR-7 |

## Functional Requirements

Conventions used below:

- **JSON transport**: today's answer of `/api/graphql`, `200 application/json` with the execution result.
- **Event stream**: the answer of FR-7 and FR-8.
- **Negotiates SSE**: the rule of FR-2.
- **Execution result**: the JSON object async-graphql serialises for a response. It always has `data` (`null` for a request error), has `errors` only when there is at least one, and omits empty `extensions`.
- **Problem**: the RFC 9457 document of `shared_application::http::Problem`, `application/problem+json`, as in bundle 4.
- **Event ids, cursors and type names**: as in the conventions of 4/FR. The event at Kafka offset `o` has the id `o + 1` in decimal. A cursor is a decimal unsigned 64-bit integer. A type name has 1 to 128 characters and matches `^[a-z][a-z0-9]*([._-][a-z0-9]+)*$`.
- **Hub subscriber**: a registration in the node's hub. It counts in `subscribers` (status route and `eventStreamStatus`) and against `MAX_SUBSCRIBERS`.
- `\n` in byte sequences is one line feed (LF).

### Schema

**FR-1 Subscription root and event object.** The schema gains the subscription root `Subscription` with exactly one field, `events`, the object type `Event` and the scalar `JSON`. The query root is unchanged, and no mutation root is added. In the sorted schema definition language that `sdl()` prints and the snapshot holds:

```graphql
"An event of a subscribed type: the same object as a WebSocket frame."
type Event {
  "The event object the producer wrote: envelope id, type, occurredAt and data."
  data: JSON!
  "Event id: the Kafka offset of the event plus one, in decimal; the cursor of a resume."
  id: String!
  "Event type name."
  type: String!
}

"A scalar that can represent any JSON value."
scalar JSON

"Subscription root."
type Subscription {
  "Streams every event of the listed types (1 to 100 names); with lastEventId or the Last-Event-ID header, every retained event after that id comes first."
  events(lastEventId: String, types: [String!]!): Event!
}

schema {
  query: Query
  subscription: Subscription
}
```

- `Event.id` is the event id, the value of the `id` field of the SSE route and of the WebSocket frame for the same event.
- `Event.type` is the event type name.
- `Event.data` is the event object (4/FR-18): envelope `id`, `type`, `occurredAt` and `data`. It is equal, as a JSON value with its members in the same order, to the `data` line of the SSE route and to the `data` member of the WebSocket frame for the same event.
- `JSON` is the async-graphql scalar of that name.
- `Event.data` has the field complexity 501, more than half of `MAX_COMPLEXITY` (1 000); `Event.id` and `Event.type` keep the default cost of 1. A document that selects `data` more than once, under several aliases, under one name written twice or through fragments, exceeds the complexity limit and is answered as a document error (FR-15) before any resolver runs. The selection `events(..) { id type data }` costs 504 and runs. The complexity is not part of the SDL.
- Every description is one line, and no description holds `node`, `Connection`, `update`, `delete` or `input `, the words the schema test refuses. The arguments carry no description, so the signature stays on one line. A later rewording changes the schema test and the snapshot in the same story.
- `.github/graphql-spec/spec` is regenerated with `make.generate_graphql_spec` in the story that changes the schema. `make.validate_contract_snapshots` reports no drift, and `make.graphql_diff` reports only non-breaking changes, so no `approved-breaking-change` label is needed.

> Assumption: the field is `events(types: [String!]!, lastEventId: String): Event!` and the object is `Event { id, type, data }`, because that is the frame of the WebSocket route (4/FR-9) that the issue names "the event object (id, type and the envelope data)"; one shape on every transport lets a client share its parser. The sorted SDL prints the arguments in the order `lastEventId`, `types`; the order is not part of the API.

> Assumption: `Event.id` is a `String!` holding the decimal id, not an `Int` or `ID`, because event ids are 64-bit and exceed GraphQL `Int` and the integers a JavaScript number holds exactly; the status object already writes its 64-bit values as decimal strings.

> Assumption: `Event.data` is one `JSON!` scalar rather than a typed envelope object, because the producer's `data` member is any JSON value and needs a JSON scalar anyway, and a single scalar keeps the value identical to the SSE and WebSocket forms. A typed envelope is a possible later addition and would not break this field.

> Assumption: `Event.data` costs 501 so that one execution result holds at most one copy of the event object. Every other selected field costs 1, and FR-10 allows aliases and fragments, so without this cost a document such as `{ events(..) { d1: data d2: data ... d999: data } }` passes the limit of 1 000. async-graphql resolves `Json<T>` with one `to_value` per selection, so such a document would build 999 copies of every event of its type, per subscriber, in the node's shared memory: about 64 MiB per event with the default `EVENT_MAX_DATA_BYTES`, about 1 GiB at its maximum. A cost above half the limit refuses every document that selects `data` twice, keeps `{ id type data }` valid, and needs neither a new variable nor a new rule of the service. Repeating `id` or `type` costs at most 1 000 short strings per event and stays bounded.

> Assumption: the subscription root has one field. Narrower subscriptions (by project, by envelope member) are out of scope as in bundle 4.

### Choosing the transport

**FR-2 Content negotiation.** A request to `/api/graphql` negotiates SSE when its `Accept` header ranks `text/event-stream` above the other representations of its method. Every other request uses the JSON transport or keeps its answer of today (FR-5). The rule:

- The values of every `Accept` field of the request, joined in order, are read as a comma-separated list of media ranges. Type and subtype compare without regard to letter case. Parameters other than `q` are ignored. `q` defaults to 1, and a range whose `q` is not a valid qvalue (0 to 1 with at most three decimals) is ignored. A header value that is not visible ASCII text counts as absent.
- `s` is the highest `q` of a range `text/event-stream`. `j` is the highest `q` of a range `application/json` or `application/graphql-response+json`. `w` is the highest `q` of a range `*/*` or `text/*`. `h` is the highest `q` of a range `text/html`. A range that is not listed has `q` 0.
- **POST** negotiates SSE when `s > j`. Wildcard ranges count for neither side, because a POST has two representations and JSON is today's default. Because `j` is never negative, `s > j` also means `s > 0`, and the code tests only `s > j`.
- **GET** negotiates SSE when an `Accept` field is present and `max(s, w) > max(j, h)`. A GET has no JSON representation, so a wildcard admits the event stream; a GET without any `Accept` field keeps today's answer, which GraphQL over HTTP (section "Accept") allows: a server may answer a request without `Accept` with an error or a media type of its choice; `text/html` ranked at least as high keeps the explorer in `dev` and the 405 elsewhere.

These headers are tested with the answer the rule gives:

| `Accept` | Sent by | POST | GET |
| --- | --- | --- | --- |
| absent | curl without a header, today's tests | JSON | today's answer |
| `text/event-stream` | graphql-sse client, browser `EventSource` | event stream | event stream |
| `text/event-stream, multipart/mixed` | urql subscription with `fetchSubscriptions` | event stream | event stream |
| `application/graphql-response+json, application/graphql+json, application/json, text/event-stream, multipart/mixed` | urql query | JSON | today's answer |
| `*/*` | curl, generic clients, Schemathesis | JSON | event stream |
| `text/*` | generic clients | JSON | event stream |
| `text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8` | browser navigation | JSON | today's answer |
| `application/json` | JSON clients | JSON | today's answer |
| `application/json;q=0.5, text/event-stream` | any | event stream | event stream |
| `text/event-stream;q=0` | any | JSON | today's answer |
| `TEXT/Event-Stream; charset=utf-8` | any | event stream | event stream |
| `text/event-stream;q=2` (invalid qvalue) | any | JSON | today's answer |

"Today's answer" of a GET is the explorer in `dev` and the 405 problem with `Allow: POST` elsewhere (FR-5).

> Assumption: a GET whose `Accept` admits the event stream only through a wildcard (`*/*`, `text/*`) negotiates SSE, unlike a POST. RFC 9110 12.5.1 lets a wildcard accept every type, and outside the explorer the event stream is the only representation a GET of `/api/graphql` has, so serving it is proactive negotiation and not a guess. It is also the root-cause fix for Schemathesis (FR-29): Schemathesis sends `Accept: */*`, and a 405 to a schema-valid GET would fail its check `positive_data_acceptance`. A GET without any `Accept` field keeps today's 405 (or the explorer in `dev`): GraphQL over HTTP (section "Accept") lets a server answer a request without `Accept` with an error or any media type, a client that wants a stream says so, and keeping the answer leaves every caller of today unchanged; the router test `the_explorer_exists_only_in_dev` and `features/graphql.feature` send no `Accept` and stay unchanged. A browser navigation ranks `text/html` above its `*/*;q=0.8`, so it keeps the explorer in `dev` and the 405 elsewhere. In `dev`, curl with its default `*/*` now gets an event stream (the 400 problem of a missing `query` when it sends none) instead of the explorer; the documents say so.

> Assumption: the endpoint is `/api/graphql` itself and not a sibling path, because the issue sends both the subscription and the query "to the GraphQL endpoint with `Accept: text/event-stream`", and the graphql-sse client, urql and GraphQL Yoga use one URL for both transports.

> Assumption: `Accept` is parsed as media ranges with q values and is not compared as one string. The reference handler streams only on an exact `text/event-stream`, which would break urql's subscriptions (`text/event-stream, multipart/mixed`). A "contains" test would stream urql's queries, which list `text/event-stream` with the same q as the JSON types. GraphQL over HTTP 4.2 asks the server to use the highest-priority type it supports. A tie goes to JSON because JSON is today's default and urql's queries rely on it. On POST, wildcards do not choose the event stream, so no existing POST client changes transport; the GET rule, where JSON is not a representation, follows below.

> Assumption: only the distinct connections mode of graphql-sse is served. Its single connection mode keeps a reservation in the memory of one node across a PUT, the stream, every POST and a DELETE. That needs sticky sessions or shared state, which ADR 0013 and ADR 0014 exclude. The protocol's recipes say the distinct mode suffices for most uses.

**FR-3 The POST form.** `POST /api/graphql` takes a body of media type `application/json` holding one JSON object with the members `query` (string, required), `operationName` (string or null), `variables` (object or null) and `extensions` (object or null, accepted and ignored). The body rules, the body limit `HTTP_MAX_BODY_BYTES` and the problems are those of today and are the same on both transports. A POST that negotiates SSE and is refused for its body gets the problem that the same POST without SSE gets, and no stream (FR-6). One request carries one operation. A JSON array (a batch) is refused as today.

> Assumption: the POST form is the GraphQL over HTTP request that the specification requires every server to support (4.4). The graphql-sse client always sends it, with `content-type: application/json` and exactly `accept: text/event-stream`.

**FR-4 The GET form.** A `GET /api/graphql` that negotiates SSE (the GET rule of FR-2) runs the operation given in its query string, in every `APP_ENV`. The query string is `application/x-www-form-urlencoded` with the parameters:

- `query`, the document, required;
- `operationName`, optional;
- `variables` and `extensions`, optional, each a JSON-encoded object.

Unknown parameters are ignored. The `Last-Event-ID` header is read as on POST (FR-11). These GET requests are refused before any stream, with a problem, in this order:

| Order | Status | Problem | When |
| --- | --- | --- | --- |
| 1 | 414 | type `about:blank`, `detail` `The query string is too long.` | the query string, as received, is longer than `HTTP_MAX_BODY_BYTES` bytes |
| 2 | 400 | type `about:blank`, `detail` the parser text, which names the parameter | the query string cannot be read (invalid percent-encoding or UTF-8), `query` is missing, or one of `query`, `operationName`, `variables`, `extensions` is given twice; an unknown parameter, also when repeated, is ignored |
| 3 | 422 | type `/problems/validation-failed`, one violation with `propertyPath` `variables` or `extensions`, `code` `format`, `message` `variables must be a JSON-encoded object` or `extensions must be a JSON-encoded object` | the parameter is present and is not a JSON object |

A GET that runs a query is answered as in FR-20. A mutation cannot run over GET, because the schema has no mutation root (FR-15).

> Assumption: the GET form exists for the browser `EventSource`, which can only send GET and cannot set headers. Its parameters are those of GraphQL over HTTP 4.3, and the graphql-sse recipes put `query` in the URL the same way. It is served in every `APP_ENV`, because `EventSource` pages run in production.

> Assumption: the query string is bounded by `HTTP_MAX_BODY_BYTES` and answered 414 beyond it, so that a document has the same byte budget in both forms and no new variable is needed. GraphQL over HTTP 5.4 names 414 for this case. Without the bound, only the HTTP server's header buffer, about 400 KiB, would limit it.

> Assumption: `variables` and `extensions` that are not JSON objects are answered with the 422 violation of the codebase's query-parameter convention: a parameter that is read but breaks a constraint is a 422 with a violation, and a parameter that cannot be read is a 400. GraphQL over HTTP's 405 for a mutation over GET is not reproduced, because no mutation can exist in this schema.

**FR-5 Requests that do not negotiate SSE.** Every request to `/api/graphql` that does not negotiate SSE keeps the status and the body it has today:

- A POST uses the JSON transport, whatever `Accept` lists. No `application/graphql-response+json` answer is introduced.
- A subscription operation on the JSON transport registers no hub subscriber and is answered `200 application/json` with `data` `null`. One that passes document validation, whatever its root shape (FR-14 is not applied on the JSON transport), gets the one error `Subscriptions are not supported on this transport.`. One that fails document validation, such as `subscription { __typename }`, gets async-graphql's validation errors as today.
- A GET that does not negotiate SSE (no `Accept` field, or one that ranks a JSON type or `text/html` at least as high as the event stream) gets, in `dev`, the explorer page with its content security policy.
- Such a GET outside `dev` gets the 405 problem with `Allow: POST`.
- Any other method gets the 405 problem. Its `Allow` header lists `GET`, `HEAD` and `POST` in every `APP_ENV`; today it lists only `POST` outside `dev`.

Every answer to a GET of `/api/graphql` carries `Vary: Accept`, the explorer and the 405 problem included. A request with a CORS `Origin` also keeps `Vary: Origin` (FR-21). The answers of the POST JSON transport gain no header.

> Assumption: a GET without SSE outside `dev` keeps the 405 problem with `Allow: POST`. GraphQL over HTTP uses this answer for a GET that the server will not run although it serves GET otherwise (a mutation over GET): POST is the method that answers a request for JSON. The existing router test and `features/graphql.feature` stay valid. JSON queries over GET are not added.

> Assumption: the 405 answer to a GET that does not negotiate SSE keeps `Allow: POST` although GET serves event streams. This deliberately deviates from RFC 9110 §15.5.6, which asks a 405 to list every method the resource supports, to keep today's answer (router test `the_explorer_exists_only_in_dev`, `features/graphql.feature`); `docs/streaming-protocol.md` documents the event-stream form of GET.

> Assumption: the `Allow` header of the other methods lists `GET, HEAD, POST`, so that the methods the router serves are listed: axum writes it from the registered methods, and GET now serves event streams in every environment. It is the one header of a non-SSE answer whose value changes. `features/http_errors.feature` ("allow should contain POST") still holds.

> Assumption: `Vary: Accept` is added to every answer of GET `/api/graphql`, because the representation now depends on `Accept` (RFC 9110 12.5.5). Without it a shared cache could hand the stored 405 or explorer page to an `EventSource` request for the same URL. POST answers are not reused by caches without explicit freshness, which the service never sends, so the JSON transport stays unchanged. The brief's S8 states the same: same status and body; only `Vary` on GET answers and `Allow` on other methods change.

**FR-6 The order of the answers.** A request that negotiates SSE is answered by the first rule that applies:

1. A CORS preflight (`OPTIONS`) is answered by the CORS layer (FR-21).
2. A transport error is answered with its problem and no stream: on POST, the 415, 413, 400 and 422 problems of today, in today's order; on GET, the 414, 400 and 422 problems of FR-4.
3. A document error (FR-15) or a breach of the one-root-field rule (FR-14) is answered inside an event stream.
4. Invalid arguments of `events` (FR-13) are answered inside an event stream.
5. A node that is shutting down, starting or at its cap answers the 503 problem of FR-16.
6. Otherwise the event stream opens.

At any point before the head, the request budget may expire. The answer is then the existing 503 problem `The request exceeded its time budget.` (FR-9). This order matches the REST streaming routes (4/FR-16): validation answers come before availability answers.

### The event stream

**FR-7 Response head.** Every event stream, of a subscription, a query or an error result, in either form, starts with status `200` and these headers:

- `content-type: text/event-stream`, with no parameter;
- `cache-control: no-cache`;
- `x-accel-buffering: no`;
- `vary: accept` (FR-5);
- the headers of every response of the router (request id, security headers, CORS headers for an allowed origin).

The handler sets no `connection`, `content-encoding`, `content-length` or `transfer-encoding` header.

> Assumption: the head mirrors the SSE route, `content-type: text/event-stream`, `cache-control: no-cache` and `x-accel-buffering: no`, so proxies treat both streams alike. The SSE format is always UTF-8 (WHATWG), so the reference's `charset=utf-8` adds nothing. A `connection` header is forbidden in HTTP/2. `content-encoding: none` is not a registered coding, and the router has no compression layer.

**FR-8 Events on the wire.** The body of an event stream is a sequence of blocks, UTF-8 with LF line ends, each ending with an empty line. Exactly these blocks are written:

| Block | Bytes | When |
| --- | --- | --- |
| keep-alive comment | `:\n\n` | first block of every event stream; then as in FR-17 |
| subscription result | `event: next\nid: <event id>\ndata: <result>\n\n` | once per delivered event (FR-10) |
| single result | `event: next\ndata: <result>\n\n` | the result of a query (FR-20), or an error result (FR-13, FR-14, FR-15) |
| complete | `event: complete\ndata:\n\n` | directly after a single result, followed by the end of the body; never in a subscription stream |

- `<result>` is the execution result as compact JSON on one line. Compact JSON escapes every line break inside strings, so a client that reads only the first `data:` line of a block reads the whole result.
- `<event id>` is the event id in decimal, without leading zeros.
- No other event name is ever written, so an event name never equals an event type name, and no `retry:` field is ever written.

> Assumption: `next` and `complete` are the only events of the graphql-sse protocol. Its client treats any other event name as fatal ("Invalid stream event"), so the GraphQL stream never reuses the event-type names of the SSE route.

> Assumption: `complete` carries a bare `data:` line, as the protocol requires. An `EventSource` does not dispatch an event whose data buffer is empty, and a `data:` line adds a line feed to that buffer. urql does not read a `data:` line without a value as a result, which it would do with `data: null`. Because axum's `Event::data("")` writes no `data:` line, the stream is written as bytes and not through `axum::response::sse`.

> Assumption: the `id:` line appears on subscription results only. A browser `EventSource` stores it and sends it back as `Last-Event-ID` when it reconnects, while the graphql-sse and urql parsers ignore it harmlessly. A single result carries none, so a query or an error result never changes a client's last event id.

> Assumption: no `retry:` field is written. The graphql-sse client ignores it and has its own randomised backoff. For `EventSource`, a fixed server delay would bring the clients of a node back together without the random part of the documented reconnect procedure. The SSE route writes none either.

**FR-9 When the head is written.** The handler writes the head only after the operation has reached one of three points:

- (a) the `events` field registered its hub subscriber;
- (b) the operation produced its first execution result: the result of a query, or an error result;
- (c) the hub refused the subscriber, which is answered with the 503 problem of FR-16 instead of a head.

This phase runs inside `HTTP_REQUEST_TIMEOUT`. When the budget expires first, the answer is the existing 503 problem `The request exceeded its time budget.`, and no hub subscriber remains registered. The phase never reads the broker: a subscriber is registered without a broker read, and its replay runs after the head, inside the stream (4/FR-12). A query runs completely inside the phase.

> Assumption: async-graphql runs a subscription lazily, so a refusal or an argument error shows only when the operation is first driven. Driving the operation to one of the three points before the head is the only way to answer a refusal with a 503 and to keep a query inside the request budget, which bounds only the time to the response head. "Pending at the first poll" is not a usable signal, because a subscription yields cooperatively on idle steps.

### The subscription field

**FR-10 Delivery of the `events` subscription.** A subscription operation whose root field is `events(types: T, lastEventId: c)` registers one hub subscriber for the distinct names of `T`, under the rules of 4/FR-5. It delivers, as subscription results (FR-8), every event of a listed type and no event of another type.

- The result of an event is `{"data":{"<response name>":{...}}}`. The object holds the selected fields of `Event` under their aliases, in the order of the selection set, and `<response name>` is the field's alias or `events`. Fragments and the `@skip` and `@include` directives work inside the selection set as in a query.
- The `id:` line of the block is the event id, whatever the selection set holds, also when `id` is not selected or is aliased.
- Each event is resolved against the selection set in the subscriber's own connection task (NFR-1).

For the event at offset 1042 of type `user.created`, with the selection `{ id type data }`, the block is:

```text
event: next
id: 1043
data: {"data":{"events":{"id":"1043","type":"user.created","data":{"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"userId":"5f2c"}}}}}

```

> Assumption: the transport learns the id of each result from per-request data that the resolver fills before it yields the result, and not from the client's selection set, which may omit or alias `id`.

**FR-11 Resume.** The cursor of the subscription is the `Last-Event-ID` request header when it is present, else the `lastEventId` argument, else none. Every carrier that is present must hold a cursor: a malformed `lastEventId` argument is refused also beside a valid header, and when both hold a cursor the header wins (4/FR-12). Both forms read the header.

- A subscription with cursor `c` receives every retained event of its types with an id above `c`, in id order, then live events, with no gap and no duplicate between the phases. The events come from the window, from Kafka, or after the wait of a cursor ahead of the tail, exactly as 4/FR-12 to 4/FR-15 specify, including the cap `EVENT_REPLAY_MAX_EVENTS`, the replay permits, the pages and the clamp to the end of the topic.
- A subscription without a cursor receives live events only: those with an id above the node's tail position when the subscriber was registered.
- On one connection the ids of the subscription results strictly increase, also after the subscriber fell behind and caught up (4/FR-24). No id is written twice, and no id at or below `c` is written.
- The same holds when the client resumes on another node of the same topic.
- A query ignores the header.

> Assumption: the `Last-Event-ID` header wins over the `lastEventId` argument, as on the REST streaming routes (4/FR-12). An `EventSource` reconnects with its original URL, whose document may still hold an old argument, plus a newer header. The graphql-sse client re-sends its first body on a retry, and the application adds the header through the client's `headers` function, which the client evaluates on every attempt.

**FR-12 Delivery guarantee.** The GraphQL subscription has the guarantee of 4/FR-23 and ADR 0017: at least once, in offset order, for the types of the subscription, across reconnects to any node, for retained events. The service removes no duplicates. The client deduplicates by `Event.data.id`, the envelope id, and not by `Event.id`, which is the position of the event.

### Errors

**FR-13 Invalid arguments.** When the arguments of `events` are invalid, the event stream carries one single result `{"data":null,"errors":[E]}`, then `complete`, then the end of the body. No hub subscriber is registered, and `subscribers` does not change. `E` has the `message` `The request failed validation.`, `extensions.code` `VALIDATION_FAILED` and `extensions.violations` with exactly one entry:

| Input | `propertyPath` | `code` | `message` |
| --- | --- | --- | --- |
| `types: []` | `types` | `required` | `types must hold 1 to 100 event type names` |
| `types` holding an empty name | `types` | `required` | `types must not hold an empty event type name` |
| a name longer than 128 characters | `types` | `length` | `An event type must contain between 1 and 128 characters.` |
| a name that breaks the pattern | `types` | `pattern` | `An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$.` |
| more than 100 distinct names | `types` | `max_items` | `types must hold 1 to 100 event type names` |
| a `Last-Event-ID` header that is not a cursor (text, sign, blank, overflow) | `Last-Event-ID` | `format` | `Last-Event-ID must be a decimal unsigned 64-bit integer` |
| a `lastEventId` argument that is not a cursor, also beside a valid header (the header is checked first) | `lastEventId` | `format` | `lastEventId must be a decimal unsigned 64-bit integer` |

- The names are checked in list order and the first fault answers. A repeated name counts once.
- The types are checked before the cursor, and only the first failing check answers.
- No message echoes a value of the request.

> Assumption: invalid arguments are answered inside an accepted event stream, because the graphql-sse protocol requires validation that runs before execution to report through an accepted SSE connection: a 4xx makes an `EventSource` fail without telling the page why, and makes the graphql-sse client retry a request that cannot succeed. "No stream of events is opened" (AC4) therefore means that no hub subscriber and no event exist; the stream carries only the error result.

> Assumption: the type rules, codes and messages are those of the REST routes, so a client meets one rule. A malformed cursor, a 400 on the REST routes, becomes a violation with the code `format` on the first faulty carrier, the header then the argument, so that it gets `VALIDATION_FAILED` as AC4 asks. The resolver builds a 422 problem and converts it with `problem_to_error`, so the shared helper stays unchanged.

**FR-14 One root field.** A subscription operation must satisfy the direct-field rule:

- The root selection set of the operation that runs holds exactly one selection. It is a field selection written directly at the root: no fragment spread or inline fragment at the root, even one that expands to one field. The same response name written twice counts as two selections.
- Its name does not start with `__`.
- It carries no `@skip` or `@include`.

Fragments and directives inside the selection set of that field work as FR-10 says. The rule is checked for the operation that runs, which `operationName` selects. When the operation breaks a rule, the event stream carries one single result `{"data":null,"errors":[E]}`, then `complete`, then the end. No resolver runs, and no hub subscriber is registered. `E` has the `message` `The request failed validation.`, `extensions.code` `VALIDATION_FAILED` and one violation with `propertyPath` `query`, `code` `single_root_field` and `message` `a subscription operation must select exactly one root field`.

> Assumption: the rule is enforced by the service, because async-graphql has no single-root-field validation rule. Without it, `subscription { a: events(..) b: events(..) }` would register two hub subscribers for one request and interleave two id sequences on one stream.

> Assumption: the rule is the direct-field rule and not the wording of GraphQL specification 5.2.3.1 (one response name after fragments are expanded), because async-graphql 7.2.1's `collect_subscription_streams` makes one stream per direct `Selection::Field` of the root and skips root fragments (`async-graphql-7.2.1/src/subscription.rs:43-44`). Under the specification's wording, `subscription { ...F }` would pass and run nothing, a 200 stream that opens no subscriber and writes only `:` and `complete`, and `events(..) { id } events(..) { type }` would register two hub subscribers. The direct-field rule refuses more documents than the specification and never runs more or fewer than one subscriber. The ban on `@skip` and `@include` at the root keeps "one request, one subscriber" independent of variables, as the current GraphQL specification draft and graphql-js do.

**FR-15 Document errors.** A document that cannot be parsed or that fails validation is answered on the event stream as one single result `{"data":null,"errors":[...]}`, then `complete`, then the end. Validation failures include:

- an unknown field or argument, or a wrong argument type;
- a variable that cannot be coerced;
- more than one operation without `operationName`, or an unknown `operationName`;
- a depth above `MAX_DEPTH` (16) or a complexity above `MAX_COMPLEXITY` (1 000), among them a document that selects `Event.data` more than once (FR-1);
- a mutation, because the schema has no mutation root.

The errors are those async-graphql reports, with the same `message` and `locations` that the JSON transport returns for the same document. They carry no `extensions.code`, as on the JSON transport today. No hub subscriber is registered. These messages may quote parts of the client's own request (a source line of a parse error; a field, argument, variable, fragment or operation name), exactly as the JSON transport does today, and they are returned only to the requesting client (NFR-8).

> Assumption: document errors travel through the accepted stream for the reason of FR-13, which is the protocol's rule. They keep the messages of the JSON transport and gain no code, so the GraphQL error model stays one model; the stable codes of AD-18 belong to resolver errors.

**FR-16 Refusal by the hub.** A valid `events` subscription on a node that cannot take a subscriber is answered with the 503 problem of the REST streaming routes instead of a head (4/FR-25): type `about:blank`, and `detail`, by precedence:

1. `the node is shutting down`;
2. `the node is starting`;
3. `the node is at its subscriber capacity`.

`subscription rejected` is logged at `warn` with that `detail`. No `text/event-stream` head and no `next` event are written, and no slot is taken.

- A GraphQL subscriber holds a slot from its registration until its connection ends, like an SSE or WebSocket subscriber. With `MAX_SUBSCRIBERS = n` held by any mix of the three transports, the next subscription request of any transport is refused, and `subscribers` counts the GraphQL subscribers.
- A query over SSE is never refused by the hub, also not on a node that is starting.

> Assumption: availability is answered before the stream, with the 503 problem the REST streaming routes answer, and not as an error result inside the stream. Availability is not a validation step of the operation, and GraphQL over HTTP 5.4 names 503 for load shedding. The graphql-sse client retries any non-2xx answer with randomised backoff, possibly on another node, while an error result followed by `complete` would end its subscription for good. `docs/streaming-protocol.md` already says a 503 is safe to retry on any node. An `EventSource` does not retry a 503 by itself; the page creates a new one, as for the SSE route.

### Lifecycle

**FR-17 Keep-alive.** Every event stream writes the comment block `:\n\n` right after the head. While the stream stays open, the server writes `:\n\n` again whenever `STREAM_KEEP_ALIVE` (default 15 s) passes without a block written, also while a replay waits for a permit or a cursor ahead of the tail waits for the topic end. A comment carries no id and does not change the client's last event id. The interval is the one variable of the existing streams. No new variable is added.

> Assumption: the first comment follows the reference handler. Some browsers (Firefox, Safari) show a fetched response only after its first body bytes, not after the head. The interval is `STREAM_KEEP_ALIVE` and not the reference's 12 s, so that every stream of the service follows one setting, which is also the socket send bound; WHATWG suggests about 15 s for legacy proxies.

**FR-18 Slow and stalled clients.** A GraphQL subscriber is a hub subscriber, so 4/FR-24 applies to it unchanged:

- Publishing never waits for it. When its queue of `SUBSCRIBER_BUFFER` events is full, it falls behind and catches up from the window or from Kafka on its own connection, in order and without duplicate. Neither the reader nor another subscriber is delayed.
- A connection that accepts no data for `STREAM_KEEP_ALIVE` is ended by the socket send bound (`TCP_USER_TIMEOUT`), which applies to every accepted connection. The slot is freed when the connection ends.
- A client that reads slowly is never disconnected.
- The GraphQL resolution and serialisation of each event run in the connection's own task, so a slow resolution delays only that connection.

**FR-19 End of a stream.** A single result ends with `complete` and the end of the body (FR-8). A subscription stream never writes `complete`, and it ends in one of four cases:

- (a) The client closes the connection. The hub subscriber is dropped at once, and `subscribers` decreases.
- (b) The node shuts down (4/FR-27). The hub closes, and the stream ends after the results of the events already queued for it, without `complete`. From then on a subscription request is answered with the 503 `the node is shutting down`. The node's wait of at most 2 s for its streams includes the GraphQL streams.
- (c) A replay below the window cannot read Kafka (4/FR-14). The stream ends without `complete`, and `replay failed` is logged with the transport `graphql-sse`.
- (d) The reader loop died (4/FR-26). The node runs the shutdown of case (b).

In cases (b) to (d) the client reconnects to any node with its last id.

> Assumption: a subscription that is cut ends without `complete`, so that clients resume. The graphql-sse client retries a connection that closed while results were expected ("Connection closed while having active streams") and does not reconnect after `complete`. An `EventSource` reconnects with `Last-Event-ID` unless the page closed it, and the page closes it when it sees `complete`. `complete` is therefore written only when the operation has really finished.

**FR-20 Queries and mutations over SSE.** A query sent with SSE negotiated, in either form, runs before the head (FR-9). It is answered with the head, the comment, exactly one `next` with its execution result, then `complete`, then the end of the body.

- The result is the execution result the JSON transport answers for the same request in the same state, field errors included (for example the `VALIDATION_FAILED` of the `eventTypes` page limits).
- Introspection queries are answered the same way.
- A mutation cannot run, because the schema has no mutation root. It is answered as a document error (FR-15).
- A query is never refused by the hub.

> Assumption: queries are accepted over SSE because AC7 asks for it and the reference handler does the same: one result becomes one `next`, then `complete`. Running the query before the head keeps it inside the request budget.

### Browser access

**FR-21 Browser origins.** The allow-list of `CORS_ALLOWED_ORIGINS` (4/FR-28) governs the GraphQL subscription unchanged.

With the variable listing origin `O`:

- The preflight of the POST form (`OPTIONS /api/graphql` with `Origin: O`, `Access-Control-Request-Method: POST` and `Access-Control-Request-Headers: content-type` or `content-type, last-event-id`) answers 200 with `Access-Control-Allow-Origin: O`, the methods `GET` and `POST`, the request headers `Content-Type` and `Last-Event-ID`, and a maximum age of 600 s.
- The event-stream head of both forms, the 503 problem of FR-16 and the transport problems of FR-6 carry `Access-Control-Allow-Origin: O` and `Vary: Origin` for a request from `O`.
- The GET form, as an `EventSource` sends it (`Accept: text/event-stream` and, on reconnect, `Last-Event-ID`, both set by the browser), needs no preflight.
- A request of another origin is served without `Access-Control-Allow-Origin`. With `*`, every answer carries `Access-Control-Allow-Origin: *`.
- No credentials are allowed and no response header is exposed.

Without the variable, no answer carries a CORS header, so a page of another origin cannot read the stream. An `OPTIONS` request gets the 405 problem, as today.

> Assumption: CORS needs no code change. The allow-list already allows `GET`, `POST`, `content-type` and `last-event-id`. The `Accept` values of the reference clients are CORS-safelisted (at most 128 bytes, no unsafe byte). Without the variable the router has no CORS layer at all. Credentials are not offered, because there is no subscriber authorization; the graphql-sse client's default `credentials: 'same-origin'` and an `EventSource` without `withCredentials` both work.

### Observability

**FR-22 Logs and counters.** The GraphQL form writes the fixed log entries of 4/NFR-15 and no new message:

- `subscription rejected` (`warn`, the `detail`) for every refusal of FR-16;
- `replay failed` (`warn`, `transport` `graphql-sse`, the error chain) for case (c) of FR-19.

An answer of FR-13, FR-14 or FR-15 writes no entry, like a 422 of the REST routes. No entry holds the document, the variables, a type name or cursor taken from the request, or event data. The access log records the path only, so the document of a GET is not logged. `subscribers` and its GraphQL field count GraphQL subscribers.

### Tests, load test and documentation

**FR-23 Tests.** The behaviour above is proved at every level, with the gates of NFR-10 to NFR-15:

- **Unit tests of the composition root:**
  - the negotiation table of FR-2, for POST and for GET;
  - the GET parameters and the refusals of FR-4 (414, 400, 422), and their order;
  - the wire blocks of FR-8, byte for byte: a subscription result with `id:`, a single result without it, `complete` with a bare `data:`, the comment, and a result whose strings hold line breaks, quotes and characters outside the Basic Multilingual Plane, still on one line;
  - the three points of FR-9, with paused time: registered, then head; refused, then 503; first result, then head; budget expired, then 503 with no subscriber left;
  - the keep-alive timing of FR-17, with paused time;
  - the end without `complete` on a closed hub;
  - every case of FR-14;
  - a document that selects `Event.data` twice answered with the complexity error, and `{ id type data }` running;
  - no log entry for a query, a document error, an invalid argument and a subscription that ends normally (FR-22, NFR-9);
  - unchanged JSON-transport answers (FR-5).
- **Unit tests of the event-stream application crate:**
  - every row of FR-13;
  - the header winning over the argument;
  - a valid subscription registering exactly one hub subscriber;
  - dropping the stream freeing the slot;
  - `Event.data` equal to the frame's event object, and its complexity of 501;
  - the id handed to the transport for every result;
  - the hand-over at the release gate: events published between registration and release are yielded after release, without gap.
- **Router tests:**
  - GET with SSE served in `dev`, `test` and `prod`;
  - GET without SSE unchanged, with `Vary: Accept`;
  - the `Allow` header of another method;
  - a GraphQL subscription that outlives `HTTP_REQUEST_TIMEOUT` and still delivers (FR-9);
  - the CORS cases of FR-21 for both forms, with and without the variable.
- **Schema tests:** the SDL of FR-1 as `EXPECTED_SDL`; `type Subscription` allowed; the other forbidden words still refused.
- **Integration tests against the broker**, with a raw HTTP/1.1 client that can POST:
  - a served binary delivers an event of type `a` and not one of type `b` (AC2);
  - resume with the argument and with the header, from the window and from Kafka (AC3);
  - resume on a second node of the same topic;
  - a shutdown that ends the stream without `complete`, followed by a resume that misses no retained event;
  - `MAX_SUBSCRIBERS` held by a mix of SSE, WebSocket and GraphQL subscribers (FR-16).
- **End-to-end features:** a new feature `features/graphql_subscription.feature` with a scenario for every acceptance criterion, in the POST form and the GET form. The steps use POST and GET event-stream helpers in `tests/e2e/` that parse `next` and `complete` blocks and comments.
- **Tests of the OpenAPI document:** the tests of FR-27 and FR-28, in the composition root.

**FR-24 K6 scenario.** In every profile of `tests/Load/scripts/subscriptions.js`, each iteration also holds one GraphQL subscriber:

- a POST to `/api/graphql` with `Accept: text/event-stream` and `subscription { events(types: ["load.test"]) { id type data } }`;
- held for the profile's `hold` seconds by its own timeout, and tagged `test_type: 'held_subscription'`, so its duration never enters the catalog p99 budget;
- checked to have received at least one `event: next` whose data carries the type `load.test`, or, when the probed K6 returns no body for a request ended by its own timeout, checked through `subscribers` as the scenario already documents for SSE.

The `endpoints.subscriptions` budgets of `tests/Load/config.json.dist` do not change. `make.smoke_load_tests` passes. `docs/load-testing.md` describes the GraphQL subscriber.

> Assumption: the GraphQL subscribers are added within the existing budgets, which are not protected files. They show that the new transport holds under the smoke profile that `make ci` runs, and no capacity figure is claimed.

**FR-25 Documentation.** These files are updated in the same change:

- `docs/streaming-protocol.md` gets a section "GraphQL subscriptions over Server-Sent Events" with:
  - the field and the `Event` object;
  - the negotiation rule and both forms, with examples;
  - the head and the four blocks;
  - queries over SSE;
  - the answers before a stream (FR-6, FR-16) and the error results inside it (FR-13 to FR-15);
  - the cursor carriers and the header that wins;
  - the keep-alive and the end of a stream;
  - the reconnect procedure for each client:
    - the graphql-sse client: distinct connections mode, a `headers` function that adds `Last-Event-ID`, the retry attempts;
    - `EventSource`: listeners for `next` and `complete`, `close()` on `complete`, a new `EventSource` with `lastEventId` in the document or the variables after an answer other than 200;
    - urql with `fetchSubscriptions`, and Apollo through a link;
  - the HTTP/1.1 limit of 6 connections per origin, and HTTP/2 at the edge;
  - its contents list, its answers table and its limits table, extended for the GraphQL form.
- `docs/requests.http` gets a POST subscription, a POST subscription with `Last-Event-ID`, a GET subscription (URL-encoded) and a query over SSE.
- A new `docs/adr/0021-graphql-subscriptions-over-server-sent-events.md` records the protocol choices P1 to P20, and amends the sentence "with no subscription" of ADR 0007. `docs/adr/README.md` marks the amendment, as it marks 0005 superseded by 0020. ADR 0007 itself is not rewritten.
- `docs/architecture.md` describes the GraphQL transport and the phase before the head, inside the request budget.
- `docs/operations.md` adds the value `graphql-sse` to the transports of `replay failed`, and tells the edge proxy what the GraphQL stream needs: no buffering of `text/event-stream` on `/api/graphql`, an idle timeout above `STREAM_KEEP_ALIVE`, HTTP/2 recommended.
- `SECURITY.md` adds the GraphQL limits to its bounded inputs (`Event.data` at most once per document included), says that the GraphQL subscription has no authorization either, and says that async-graphql's document errors may quote the client's own request back to that client, as on the JSON transport.
- `docs/load-testing.md` describes the GraphQL subscriber (FR-24).
- `workspace.dsl` changes the descriptions of the service container, the composition root, the event-stream application crate and the client relationship.
- `.templatesyncignore` gains `crates/service/src/graphql/handler.rs`, and `crates/service/src/graphql.rs` when that file is edited.
- For the OpenAPI document (FR-26 to FR-29): a new `docs/adr/0022-every-route-in-the-openapi-document.md` records that every route is an operation of the document, how the streaming routes and the GraphQL endpoint are described, the route-completeness test and the Schemathesis rule; it amends ADR 0006 (`/ping` "outside the OpenAPI router") and ADR 0007, and `docs/adr/README.md` marks both amendments. `docs/architecture.md` and `docs/streaming-protocol.md` drop "outside the OpenAPI document" and point to the Swagger page. `.templatesyncignore` gains `crates/service/src/http/ping.rs`, `crates/service/src/http/ping/`, `crates/service/src/http/docs.rs` and `crates/service/src/http/docs/`, the template files and test directories these requirements edit that it does not list yet.

> Assumption: ADR 0021 amends ADR 0007 instead of rewriting it, because ADR 0007 is a template record and the index already shows how a later record changes an earlier one. ADR 0022 amends ADR 0006 and ADR 0007 the same way.

### OpenAPI document and Swagger page

**FR-26 Every route is an operation of the document.** The OpenAPI document that `make.generate_openapi_spec` writes to `.github/openapi-spec/spec.yaml`, that `GET /api/docs/openapi.json` serves and that the Swagger page at `GET /api/docs` renders holds exactly these eleven operations, one per route and method the service serves:

| Operation | Tag | Parameters and request body | Responses (every status the route can answer) |
| --- | --- | --- | --- |
| `GET /api/event-types` | EventStream | as today | as today |
| `GET /api/events/status` | EventStream | as today | as today |
| `GET /api/health` | HealthCheck | as today | as today |
| `GET /api/events/stream` | EventStream | query `types` (required), `lastEventId`; header `Last-Event-ID` | `200 text/event-stream` with the SSE frame format; 400, 422 and 503 problems |
| `GET /api/events/ws` | EventStream | query `types`, `lastEventId`; headers `Connection`, `Upgrade`, `Sec-WebSocket-Version`, `Sec-WebSocket-Key` (required), `Last-Event-ID` | `101 Switching Protocols` with the frames of both directions; 400, 403, 422, 426 and 503 problems |
| `POST /api/graphql` | GraphQL | body `application/json` `{query, operationName, variables, extensions}`; header `Last-Event-ID` | `200 application/json` (execution result) and `200 text/event-stream` (FR-8 blocks); 400, 413, 415, 422 and 503 problems |
| `GET /api/graphql` | GraphQL | query `query` (required), `operationName`, `variables`, `extensions`; header `Last-Event-ID` | `200 text/event-stream`, and `200 text/html` (the explorer, `dev` only); 400, 405, 414, 422 and 503 problems |
| `GET /ping` | HealthCheck | none | `200 text/plain` `pong` |
| `GET /api/docs` | Documentation | none | `200 text/html` |
| `GET /api/docs/openapi.json` | Documentation | none | `200 application/json` (this document) |
| `GET /api/docs/{file}` | Documentation | path `file` | `200` with the asset's media type; 400 and 404 problems |

Every operation of the table has a summary, a description, its tag, a `default` problem response (the 500 of the panic catcher and the 503 of the request budget) and:

- every parameter with its location, whether it is required, a description, a schema that admits the values the service accepts and no value it refuses, and an example;
- its request body, where it has one, with its media type, schema and example;
- every status code the route can answer, each with its media type, its schema (the problems as `application/problem+json` with the `Problem` schema) and an example;
- for `text/event-stream`, a schema of one event (`event`, `id`, `data`, the convention of OpenAPI 3.2 that Schemathesis reads), the `data` described as JSON with the schema of the frame or of the execution result, the bare `data:` of `complete` admitted, and a description of the frame format; for the 101 of the WebSocket route, the response headers `Upgrade`, `Connection` and `Sec-WebSocket-Accept` and a description of the server's text frames and of the client's commands, with an example of each.

The tags gain `GraphQL` and `Documentation`, each with a description. `API_VERSION` becomes `2.1.0`, the minor bump its documentation comment asks for when operations are added.

> Assumption: "every route the service serves" includes the three documentation routes and `/ping`, besides the routes the issue names "at least", because the issue asks that a test fail for any route served without an entry; a route left out would need its own exception in that test.

> Assumption: the documentation asset route is served and documented as `/api/docs/{file}`, one path segment, instead of today's catch-all `/api/docs/{*file}`. utoipa-axum registers the annotated path verbatim in both the router and the document, and `{*file}` is not an OpenAPI path template; the embedded Swagger UI files are one segment deep, so every asset the page loads is still served, and a deeper path gets the router's 404 problem instead of the handler's 404 problem.

> Assumption: the WebSocket handshake headers are documented as required header parameters with the values the upgrade accepts (`Connection` holding `Upgrade`, `Upgrade: websocket`, `Sec-WebSocket-Version: 13`, a `Sec-WebSocket-Key`), because they are what a client must send; without them the route answers the 400 or 426 problem of the upgrade, which is documented too. OpenAPI 3.1 has no WebSocket object, so the frames of both directions are described in the 101 response and its examples.

> Assumption: the GraphQL operations get the tag `GraphQL`, and the three documentation routes the tag `Documentation`; `/ping` joins `HealthCheck`, next to `/api/health`.

**FR-27 A route served without an entry fails a test.** Tests of the composition root prove both directions:

- Every route of the service is registered through `OpenApiRouter::routes(routes!(..))`, which adds the route to the router and its operation to the document from the same `#[utoipa::path]` annotation. A source oracle reads the non-test sources of `crates/service/src` and of every context application crate whose router `api_router()` nests, and fails on any `.route(`, `.route_service(` or `.nest_service(` call, so no route can be served outside the document.
- The document lists exactly the eleven operations of FR-26 (path and method), and the sets of tags and operation ids are exact.
- Every operation of the document is served: a request for each path and method through the assembled router of each `APP_ENV` reaches its handler, never the router's 404 fallback or its 405 answer for an undocumented method.
- The document that `GET /api/docs/openapi.json` serves equals the one `make.generate_openapi_spec` writes.

**FR-28 The Swagger page lists every route.** `GET /api/docs` keeps serving the Swagger UI page with its content security policy, and the page renders `GET /api/docs/openapi.json`, so it lists every operation of FR-26 with its parameters, request body, responses and examples. An end-to-end scenario reads `/api/docs/openapi.json` through the served router and finds each of the eleven operations.

**FR-29 The OpenAPI gates and Schemathesis pass.** In the story that changes the document, `.github/openapi-spec/spec.yaml` is regenerated with `make.generate_openapi_spec`, and:

- `make.validate_contract_snapshots` reports no drift;
- `make.validate_openapi_spec` (Spectral with the protected `.spectral.yaml`) reports no finding;
- `make.openapi_diff` reports no breaking change, so no `approved-breaking-change` label is needed: the change adds operations, tags and schemas and removes nothing;
- `make.schemathesis_validate` (`--checks all --phases=examples,coverage` against the production image in the load-test stack, with the protected flags as amended by the approved `71aab46`) passes with ten of the eleven operations of FR-26 tested, the other streaming ones included; `GET /api/events/ws` is excluded only by that approved change and stays documented on the Swagger page.

A Schemathesis failure on a streaming operation is fixed at its root: in the document (a schema that admits a value the service refuses, a missing status code or header, an event schema that does not match the frames) or in the code (an answer that contradicts the documented contract). Apart from the approved exclusion of `GET /api/events/ws` (`71aab46`), no operation is excluded from Schemathesis and no check is disabled or relaxed. Should a failure be impossible to fix at its root, the story stops and escalates to the maintainer with the failing check, the reason and the exact exclusion that would remove it (where it lives, which operation, which check); an exclusion in a protected file such as `make/gates.mk` goes through the label `approved-config-change`, and the operation stays documented on the Swagger page either way.

> Assumption: no exclusion is planned, because Schemathesis 4.33.1 can test every operation of FR-26 (`research.md` section 3.11): it reads an event stream until the stream ends, 20 events arrive or its 10 s request timeout passes, and treats a stream cut short as a normal response; it validates each event against a `text/event-stream` schema of one event; it sends `Accept: */*`, which the GET rule of FR-2 answers with an event stream instead of the 405 that its check `positive_data_acceptance` would refuse; and it accepts a documented 101. The escalation path is the issue's own rule for a failure that cannot be fixed at its root. Amended on 2026-11-20: the run on `4dfd92e` showed that Schemathesis reports the valid handshake's 101 as a failure (ST101, ST202); the escalation path ran as written, and the maintainer approved the exclusion of `GET /api/events/ws` alone (`71aab46`).

### Traceability

| Issue criterion | FR ids | Brief |
| --- | --- | --- |
| AC1. The snapshot has a `Subscription` root whose field returns the event object of the listed types; `make graphql-diff` reports no breaking change | FR-1, NFR-16 | S1 |
| AC2. A subscription for type `a` with `Accept: text/event-stream` answers 200 `text/event-stream`; an event of type `a` arrives as one `next` with `{"data":{...}}`; an event of another type does not | FR-2, FR-3, FR-4, FR-7, FR-8, FR-9, FR-10 | S2, S3 |
| AC3. Resume with a last event id: every retained event after it, in order, before live events, never the same id twice | FR-11, FR-12, FR-19 | S4, S10 |
| AC4. An invalid type name, more than 100 types or a malformed last event id gives `VALIDATION_FAILED`, and no stream of events is opened | FR-13, FR-14, FR-15, FR-6 | S5 |
| AC5. Starting, at capacity or shutting down: refused before a stream with the reason; counts against `MAX_SUBSCRIBERS` | FR-16, FR-9, FR-19 | S6, S10 |
| AC6. A comment every `STREAM_KEEP_ALIVE` without events; a client that stops reading is disconnected without delaying others or the reader | FR-17, FR-18, NFR-6 | S7 |
| AC7. A query with `Accept: text/event-stream` answers one `next`, then `complete` | FR-20, FR-8 | S8 |
| AC8. A page of an origin of `CORS_ALLOWED_ORIGINS` can open the subscription; without the variable no other origin can | FR-21, FR-4 | S9 |
| AC9. The OpenAPI document and the Swagger page list every route (at least the seven named), each with its parameters, request body, every status code (`200 text/event-stream` or `101` with the frame format, the 400, 403, 422 and 503 problems) and examples; a test fails when a route is served without an entry | FR-26, FR-27, FR-28, FR-25 (ADR 0022) | S13 |
| AC10. `make validate-openapi-spec`, `make openapi-diff` (no breaking change) and `make schemathesis-validate` pass; a streaming endpoint is excluded from Schemathesis only when it cannot be tested, with the reason, and stays on the Swagger page | FR-29, FR-2 (GET rule), NFR-16, NFR-20 | S14 |
| AC11. `docs/streaming-protocol.md` and `docs/requests.http` describe it; every local gate of `make ci` passes, 100 % coverage, no surviving mutant | FR-23, FR-24, FR-25, NFR-10, NFR-11, NFR-12, NFR-13, NFR-14, NFR-15, NFR-16, NFR-19, NFR-21 | S11, S12 |
| M1. graphql-sse over SSE with GraphQL over HTTP formats, every protocol choice recorded | FR-2 to FR-8, FR-25 (ADR 0021) | S11 |
| M4. (2026-11-20) RUSTSEC-2026-0542 and RUSTSEC-2026-0543 ignored in `deny.toml` by the two approved entries; humantime replaced in the two service crates in this pull request, keeping the documented duration syntax and the envelope time rule | NFR-21, NFR-15, NFR-17 | S15 |

## Non-Functional Requirements

### Performance

**NFR-1 Publish path unchanged.** The cost of publishing an event stays as 4/NFR-2: one frame per event, shared by its subscribers whatever their transport. Two rules keep it so:

- A representation that the GraphQL form needs and that is computed on the publish path is computed at most once per event and shared, and only for an event whose type has a subscriber on the node.
- The per-subscriber work runs in the subscriber's connection task, after the frame left the hub queue, and never on the publish path: taking the event object, resolving the selection set, serialising the result.

An event of a type without a GraphQL subscriber costs nothing more than today.

**NFR-2 Time to the head.** No answer before the head waits for a broker read (4/NFR-3): registering a subscriber reads nothing, and the replay runs after the head. The phase before the head (FR-9) is bounded by `HTTP_REQUEST_TIMEOUT`. A unit test with a broker fake that never answers shows that a subscription with a cursor below the window receives its head and then its keep-alives.

**NFR-3 Bounded memory.** A GraphQL subscriber holds what an SSE subscriber holds (4/NFR-9), plus at most one execution result being written. That result holds at most one copy of the event object plus its id and type, because a document selects `Event.data` at most once (FR-1). The request it came from is bounded by `HTTP_MAX_BODY_BYTES`, as a body or as a query string, and the document by its depth and complexity limits. No GraphQL state outlives its connection, and nothing is kept between requests.

**NFR-4 Smoke budget and capacity target.** `make.smoke_load_tests` passes with the GraphQL subscribers of FR-24, within the unchanged `endpoints.subscriptions` budgets and the existing success rate of the checks (above 99 %). The design target of 4/NFR-1 is unchanged. It counts GraphQL subscribers among the subscribers of a node, and this change does not measure it.

### Availability and resilience

**NFR-5 Stateless nodes.** The GraphQL form keeps the nodes stateless (ADR 0013, ADR 0014). Each request is self-contained in the distinct connections mode, and no reservation, token or operation id is kept between requests. Any node serves any request, and a client resumes on any node with its last id (4/NFR-7). A 503 is safe to retry on any node.

**NFR-6 Isolation.** A GraphQL subscriber that cannot keep up affects only itself (4/NFR-10). A test shows that a GraphQL subscriber that takes nothing delays neither the delivery to another subscriber of the same type nor the tail. The end of its stalled connection rests on the send bound set on every accepted connection, which the existing tests of `serve` prove for all transports. No GraphQL-specific socket test is needed.

### Security

**NFR-7 No subscriber authorization.** As 4/NFR-13: any client that reaches the service can subscribe through GraphQL to any type, and the topic must carry only events that are safe for every client. `SECURITY.md` and `docs/streaming-protocol.md` say so for the GraphQL form.

**NFR-8 Input limits and secrets.** Every input of the GraphQL form is bounded:

- the body by `HTTP_MAX_BODY_BYTES` (default 65 536), and the query string of a GET by the same number of bytes;
- the document by depth 16 and complexity 1 000;
- each execution result to one copy of the event object plus id and type, because `Event.data` costs 501 and is selected at most once (FR-1);
- a subscription to one root field, 100 distinct type names of at most 128 characters each, and a 64-bit cursor;
- each request to one operation;
- each node to `MAX_SUBSCRIBERS` subscribers of all transports together.

No log entry or problem holds the document, the variables, a type name or cursor of the request, event data or the value of a variable. The GraphQL errors that this service writes (FR-13, FR-14, and the wiring errors) hold no value of the request. The document errors of FR-15 keep async-graphql's messages, which may quote parts of the client's own request (a source line of a parse error; a field, argument, variable, fragment or operation name), exactly as on the JSON transport today, and they are returned only to the requesting client. `make.check_security` and `make.secrets_scan` pass.

### Observability

**NFR-9 Logs.** The fixed messages of 4/NFR-15 stay the only log messages of the streams. `replay failed` gains the transport value `graphql-sse`, and `docs/operations.md` lists it. A healthy node writes nothing for a GraphQL subscription that opens, delivers and ends normally.

### Maintainability and quality bar

**NFR-10 Coverage.** `make.tests_with_coverage` reports 100 % line and 100 % function coverage with no exclusion added. No `#[coverage(off)]` is used.

**NFR-11 Mutation.** `make.mutation_tests_diff` reports zero surviving and zero timed-out mutants in the changed code. No `#[mutants::skip]` is used, and the mutation configuration is unchanged.

**NFR-12 Static analysis and complexity.** `make.format`, `make.lint_code`, `make.static_analysis` and `make.complexity` report zero findings:

- clippy pedantic;
- cognitive complexity at most 8;
- at most 50 lines and 5 arguments per function;
- nesting at most 5.

No `#[allow]` is used, and an `#[expect]` carries a reason. Timers use tokio time, paused in tests. Tools run only through make, and the git hooks are never bypassed.

**NFR-13 Layering and dependencies.** Each new piece of code has its place:

- **The `graphql` module of the event-stream application crate** holds the subscription resolver and the `Event` object. It is, with `http`, the only module there that may import `async_graphql`. It reaches the type rule, the cursor rule and the registration of a subscriber crate-internally.
- **`crates/service/src/graphql/`, in the composition root,** holds the merged subscription root, the schema type with it, the transport (negotiation, the GET form, the phase before the head, the byte-level stream, the keep-alive timer) and the one-root-field rule.

The rest stays fixed:

- The domain and infrastructure crates of the context, and the shared crates, are not changed. `shared_application::graphql` is unchanged.
- No crate is added to `[workspace.dependencies]` except `jiff`, which replaces `humantime` there (NFR-21, story 1.4). A crate adds a workspace dependency to its own manifest only within its layer's allow-list: `jiff` goes to `crates/service` only, whose layer allows every crate, and the event-stream application crate drops `humantime` and adds nothing. The application layer uses neither `tokio-stream` nor `async-stream`. No allow-list changes.
- The GraphQL handlers `execute`, `subscribe` and `subscribe_or_explore` take `HeaderMap`, the recorded streaming exception; `subscribe` and `subscribe_or_explore` also take `RawQuery`, to bound and strictly parse the raw query string. The extractor oracle of the router tests names these two exceptions for these three handlers only.
- **The OpenAPI document (FR-26, FR-27):** the `#[utoipa::path]` annotations of the two streaming handlers and the `IntoParams` of their parameters live in the `http` module of the event-stream application crate, whose router registers them through `routes!`; the annotations of the GraphQL, liveness and documentation handlers and the merge of the three routers (REST, GraphQL, operational) live in `crates/service`.
- `make.architecture`, `make.check_crate_isolation`, `make.unused_dependencies` and `make.validate_architecture_model` pass.

**NFR-14 Tests at every level.** The tests of FR-23 use the fakes of `tests/support` for the unit level and the broker of the stack for the integration and end-to-end levels:

- Every timer test uses paused tokio time, and every awaited call in a test is bounded by a time-out, so a mutant fails an assertion and never hangs a test.
- The wire blocks are compared byte for byte, and the JSON of results is compared with `serde_json`.
- The integration and end-to-end tests run through `make.integration_tests` and `make.e2e_tests` under the serialized verify step.

**NFR-15 Protected files and the local gates.** No story edits a protected file (`protected-config.toml`). The protected changes of pull request #9 for issue #8 are exactly the four the maintainer approved under the label `approved-config-change`, each committed on its own outside the stories and recorded in `architecture.md` "Protected Changes":

- the override of Spectral's `operation-success-response` for `GET /api/events/ws` in `.spectral.yaml` (`a374895`, 2026-11-17);
- the exclusion of that operation from Schemathesis in `make/gates.mk` (`71aab46`, 2026-11-17);
- the two `[advisories] ignore` entries RUSTSEC-2026-0542 and RUSTSEC-2026-0543 in `deny.toml`, each with its reason and removal condition (`c4f83b3`, 2026-11-20, M4, NFR-21);
- the plant of `jiff` in place of `humantime` in test G8 of `tests/CLI/bats/make_quality_gates.bats`, which removing `humantime` from `[workspace.dependencies]` needed (`49641e7`, 2026-11-20, NFR-21).

Every gate of `make ci` that runs locally passes, through the stage keys `make.ci_preflight`, `make.ci_static_analysis`, `make.ci_tests`, `make.ci_contracts`, `make.ci_mutation`, `make.ci_delivery` and `make.ci_cli`.

### Compatibility

**NFR-16 Contracts.** The GraphQL snapshot is regenerated in the same story as the schema (FR-1):

- `make.validate_contract_snapshots` is clean.
- `make.graphql_diff` reports only non-breaking changes: a new subscription root, the type `Event` and the scalar `JSON`.
- The OpenAPI document gains the eight operations of FR-26 that it lacks today and loses nothing: its snapshot is regenerated with `make.generate_openapi_spec` in the story that changes it, `make.openapi_diff` reports only non-breaking changes, and `make.validate_openapi_spec` and `make.schemathesis_validate` pass (FR-29).
- The protected Bats test G20 keeps its meaning without a change to the protected file. It removes the first single-line field of the first `type` block, now `data: JSON!` of `Event`, and expects the diff to fail.
- The protected Bats tests of the OpenAPI gates keep their meaning without a change to their file: G17 removes the first operation-level `description` line, now that of `GET /api/docs`, the first path in sorted order, and still expects Spectral to fail; G19 removes the required `title` of `Problem`, which every new problem response references, and still expects `openapi-diff` to fail; G21 plants an undocumented path and still expects `schemathesis-validate` to fail, and its run ends because Schemathesis bounds every event stream (NFR-20).

**NFR-17 Existing behaviour.** Every request that does not negotiate SSE keeps its status and body (FR-5). Its headers change only by `Vary: Accept` on GET answers and by the `Allow` value of other methods. A GET whose `Accept` admits the event stream only through a wildcard now negotiates SSE (FR-2); a GET without an `Accept` field, which every existing test sends, does not. The rest is unchanged too:

- the routes `/api/events/stream` and `/api/events/ws`, their answers and their frames;
- the configuration variables, of which none is added or changed, and every duration value that `docs/environment.md` and the configuration tests name, which parses to the same `Duration` with the same range and reason after humantime leaves (NFR-21);
- the envelope's `occurredAt` rule, which accepts and refuses the same strings (NFR-21);
- the exit statuses of `serve`.

The existing tests of these behaviours pass unchanged (`features/graphql.feature`, `features/http_errors.feature`, the router tests of the JSON transport). Only the oracles named in `research.md` section 2.5 change:

- the schema test's ban on `type Subscription`, its first-type oracle, and the full-SDL oracle `EXPECTED_SDL` / `the_sdl_is_the_documented_schema`, which becomes the SDL of FR-1;
- the extractor oracle (NFR-13);
- the router-test helper `has_cors_headers`, which today counts any `vary` header as a CORS header and is narrowed to an `access-control-*` header or a `vary` value `origin`, because every GET answer now carries `vary: accept`;
- the OpenAPI oracles of `crates/service/src/openapi/tests.rs` (`documents_exactly_the_three_operations`, `leaves_the_liveness_route_and_the_docs_routes_out`, `defines_exactly_the_five_schemas`, the contract version) and the documentation comments of the event-stream router and handlers that say the streaming routes are not operations of the document, which become the oracles and texts of FR-26 and FR-27;
- the expected value of `each_kafka_duration_accepts_its_bounds` (`crates/service/src/config/tests/kafka.rs`), which `humantime::parse_duration` computes today and a `Duration` literal per case states after story 1.4 (NFR-21).

**NFR-18 Client compatibility.** The service works with these clients:

| Client | Form | Proof |
| --- | --- | --- |
| graphql-sse client, distinct connections mode (its default) | POST, exactly `accept: text/event-stream`, `content-type: application/json` | integration and end-to-end tests with that request |
| browser `EventSource` | GET, `Accept: text/event-stream`, `Last-Event-ID` on reconnect | end-to-end tests with that request; `id:` lines and the bare `data:` of `complete` tested byte for byte |
| urql with `fetchSubscriptions: true` | POST, `Accept: text/event-stream, multipart/mixed` | negotiation tests with urql's headers |
| Relay and Apollo Client through graphql-sse | POST | documented recipes |

The single connection mode of graphql-sse is not supported: its PUT reservation gets the 405 problem, and `docs/streaming-protocol.md` says to keep the client's default distinct mode.

### Documentation

**NFR-19 Documentation gate.** `make.docs_check` passes. The documents of FR-25 change in the same story as the behaviour they describe, and `docs/streaming-protocol.md` and `docs/requests.http` hold the GraphQL subscription (AC11).

### Contract testing

**NFR-20 Schemathesis tests every operation within its own bounds.** `make.schemathesis_validate` exercises every operation of FR-26 in both of its phases, except `GET /api/events/ws`, which only the approved `71aab46` excludes and which stays documented on the Swagger page, and ends without a timeout of its own: every event stream is read until it ends, 20 events arrive or 10 s pass, and no request of the run waits for a broker read before its head (NFR-2). The examples of the streaming operations subscribe to `load.test`, the type the load-test publisher writes, so an example stream reaches its 20 events instead of its timeout. The load-test stack and its publisher do not change, and the protected Schemathesis flags (`make/gates.mk`) change only by that approved exclusion.

### Supply chain

**NFR-21 No unmaintained crate in the production build.** The production build, the dependency graph of the workspace members without dev edges, holds no crate that a RustSec advisory marks unmaintained, and `make.check_security` (cargo deny with the protected `deny.toml`) passes. A crate that such an advisory marks and that only dev edges reach may stay as an entry of `[advisories] ignore` in `deny.toml`, with its reason and its removal condition, added only as a protected change the maintainer approved. For the two advisories of M4:

- RUSTSEC-2026-0542, `humantime` 2.4.0: story 1.4 removes `humantime` from `[workspace.dependencies]` and from the manifests of `crates/service` and of the event-stream application crate. `crates/service` parses its durations with `jiff` 1.0.1 (licence `Unlicense OR MIT`, which `deny.toml` admits through `MIT`; the lock also gains `jiff-core`, `jiff-static` and `portable-atomic-util`, all admitted through `MIT`), and the envelope parser checks `occurredAt` with a rule of its own that needs no crate. `humantime` then remains only through `cucumber` 0.23.0, which only the `[dev-dependencies]` of `tests/e2e` name, so the maintainer's check `cargo tree --workspace -e no-dev -i humantime` prints nothing and the reason of the `deny.toml` entry holds. The entry is removed when cucumber drops the crate.
- RUSTSEC-2026-0543, `chrono` 0.4.45: nothing changes. It enters only through the `test-util` feature of `aws-smithy-http-client`, which the dev-dependency `aws-smithy-mocks` of the health-check infrastructure crate enables, so the production binary never compiles it. The entry is removed with SQS.
- Behaviour is kept. Every duration value that `docs/environment.md`, `.env`, `.env.test` and the configuration tests name parses to the same `Duration`; the six ranges and the two reasons `must be a duration from <range>` and `must be a duration such as 500ms, 2s or 1m` are unchanged; a dot fraction such as `1.5s` reads as before. The bare `0` keeps its reading, `Duration::ZERO`. The edges that move are documented nowhere and used by no file, and each is pinned by a test: units of days and longer (`1d`, `2w`, `1M`, `1y`), units out of order (`30s1m`), a repeated unit (`1h1h`), a space before or inside a number (` 2s`, `1 0s`) and a space that is not ASCII before the unit, all accepted today, are refused with those reasons; a leading `+` (`+2s`), a comma fraction (`1,5s`) and the clock form (`01:30:00`), all refused today, are read. `occurredAt` accepts and refuses exactly the strings it does today, and the existing envelope tests pass unchanged. `docs/environment.md` names the accepted duration syntax without naming a crate.
- `make.architecture` passes with `architecture.toml` unchanged, `make.unused_dependencies` reports nothing, and `make.tests_with_coverage` and `make.mutation_tests_diff` hold the bars of NFR-10 and NFR-11 for the changed code.

> Assumption: the durations are parsed with jiff's friendly format, which reads every documented and tested value to the same `Duration` and refuses negative values, and units of days and longer are refused, because no file of the repository uses them and jiff's duration parser does not read them (`research.md` sections 9.3 and 9.4). The bare `0`, which humantime reads as zero through an explicit special case and jiff refuses for want of a unit, keeps today's reading through a one-line guard in `parse_duration`, because an operator may have set `SQS_CONNECT_TIMEOUT`, `SQS_OPERATION_TIMEOUT` or `SHUTDOWN_GRACE_PERIOD` to `0` and the node would otherwise refuse to start after an upgrade. The other moved edges take jiff's answer, because they are documented nowhere and no file uses them. The envelope keeps a rule of its own because the protected `architecture.toml` lets the application layer use `humantime` but not `jiff`, and because jiff's timestamps end at `9999-12-30T22:00:00.999999999Z` and read at most nine fraction digits, so it would refuse strings that are accepted today (`research.md` section 9.5).

## Product Scope

### In scope

- The `Subscription` root with `events`, and the `Event` object and `JSON` scalar, in the schema and the committed snapshot.
- GraphQL over SSE in the distinct connections mode on `/api/graphql`: the POST form and the GET form, negotiation by `Accept`, queries over SSE.
- Reuse of the hub, the window, the replay from Kafka, the cursor and type rules, the cap, the readiness, the shutdown and the slow-client handling of bundle 4.
- `VALIDATION_FAILED` inside the stream, the one-root-field rule, and the 503 problem before the stream.
- The keep-alive, the ends of a stream, and browser access through the existing allow-list.
- The tests of FR-23, the GraphQL subscriber of the K6 scenario, the documentation of FR-25, ADR 0021, the architecture model and the `.templatesyncignore` entries.
- Every route of the service in the OpenAPI document and on the Swagger page, the route-completeness tests, the regenerated OpenAPI snapshot with `API_VERSION` 2.1.0, ADR 0022, and the OpenAPI gates green on every operation and Schemathesis green on every operation but the approved exclusion of `GET /api/events/ws` (FR-26 to FR-29).
- `humantime` out of the production build: jiff for the durations of `crates/service`, a rule without a crate for the envelope time, the documented syntax kept, the two approved `deny.toml` entries of 2026-11-20 and the approved plant of `jiff` in the Bats test G8 (NFR-21, M4).

### Out of scope

| Item | Follow-up |
| --- | --- |
| A GraphQL WebSocket protocol (`graphql-ws`, `graphql-transport-ws`) | Excluded by the issue; the plain WebSocket route stays as it is. |
| Subscriber authorization | A later issue, as in bundle 4 (4/NFR-13). |
| The single connection mode of graphql-sse | Not planned: it needs sticky sessions or shared state (ADR 0014). HTTP/2 at the edge lifts the 6-connection limit of HTTP/1.1 instead. |
| JSON queries over GET | Not planned; POST serves queries. |
| `application/graphql-response+json` answers and their status codes on the JSON transport | A later issue; the JSON transport stays as it is. |
| Incremental delivery (`@defer`, `@stream`, `multipart/mixed`) | Not planned. |
| Running subscriptions from the development explorer | Not planned; the documentation names the clients that can. |
| Subscriptions narrower than an event type | Not planned, as in bundle 4. |
| Limits per client (connections per address) | The edge proxy limits them (4/NFR-14). |
| A GraphQL mutation | Not planned; events enter through the topic only. |
| A server-chosen reconnection delay (`retry:`) | Not planned; the documented reconnect procedure applies. |
| A write deadline for an SSE stream on an HTTP/2 connection stalled by flow control | A later issue, as in bundle 4. |
| More than one root field per subscription, or batched requests | Refused by design (FR-14, FR-3). |
| `Accept` in the CORS allow-list for values that are not safelisted | Not needed by the reference clients; a later issue if a client needs it. |
| An AsyncAPI document of the WebSocket and event-stream messages | Not planned; the OpenAPI document describes the frames in its responses (FR-26). |
| Further exclusions from Schemathesis or relaxed Schemathesis checks | Not planned; the one exclusion, `GET /api/events/ws`, is the maintainer's approved `71aab46`, and any other failure that cannot be fixed at its root is escalated to the maintainer (FR-29). |
| Replacing `cucumber` 0.23.0 or `aws-smithy-mocks`, the dev-only holders of `humantime` and `chrono` | Each `deny.toml` entry carries its removal condition: when cucumber drops humantime; with SQS. |
| Removing the unused `humantime` entry of the application layer's allow-list in the protected `architecture.toml` | A later protected change; the architecture gate does not report an unused entry. |
| Duration units of days and longer in configuration values | Not planned; no file uses them, and they are refused with the unchanged reasons (NFR-21). |

### Delivery guarantees

The guarantees listed in the Product Scope of bundle 4 apply unchanged to the GraphQL subscription. The GraphQL form is one more transport of the same hub: it adds no guarantee and removes none.
