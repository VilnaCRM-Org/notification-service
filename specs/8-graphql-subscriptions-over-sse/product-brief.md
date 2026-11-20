---
stepsCompleted: [autonomous-create-brief]
inputDocuments:
  - https://github.com/VilnaCRM-Org/notification-service/issues/8
  - specs/8-graphql-subscriptions-over-sse/research.md
date: 2026-11-16
author: bmad-autonomous-planning (create-brief stage)
---

# Product Brief: GraphQL subscriptions over Server-Sent Events

This brief covers issue #8 (read on 2026-11-16 and again on 2026-11-17, after the maintainer's decision of 2026-11-17 added AC9 and AC10) and the context `notification/event-stream` with the composition root `crates/service`.

- "AC" is the number of the issue's acceptance criterion, in the order the issue lists them on 2026-11-17, 1 to 11. AC9 (every route in the OpenAPI document and on the Swagger page) and AC10 (the OpenAPI gates and Schemathesis) come from the maintainer's decision of 2026-11-17; the former last criterion is AC11.
- "M1" is the maintainer's decision: GraphQL subscriptions travel over Server-Sent Events and follow the graphql-sse protocol and GraphQL over HTTP, with every protocol choice recorded.
- "M4" is the maintainer's decision of 2026-11-20 on two RustSec advisories that made `make check-security` fail: both are ignored in `deny.toml` by two approved entries with a reason and a removal condition, and humantime is replaced in the two service crates in this pull request (`research.md` section 9).
- The protocol choices themselves are in `research.md` section 8.

## Problem

Frontends already query the service through GraphQL (`POST /api/graphql`: `eventTypes` and `eventStreamStatus`), but they cannot receive events through it. The schema has no subscription root, so a GraphQL client has to open a second, non-GraphQL connection to `GET /api/events/stream` or `GET /api/events/ws`. This applies to Apollo, urql and Relay with the graphql-sse client. That second connection has its own frame format, its own errors and its own reconnect handling.

## Users

| User | Need |
| --- | --- |
| Frontend developers with a GraphQL client (graphql-sse, urql, Relay; Apollo through a link) | One endpoint and one schema for the catalog, the status and the live events; GraphQL errors with stable codes; resume after a lost connection |
| Browser pages without a GraphQL client library | A subscription a native `EventSource` can open and resume by itself |
| Operators | The same limits, readiness rules, subscriber cap, 503 answers and shutdown as the existing streaming routes; no new state on the nodes |
| Maintainers | Every protocol choice written down with its reason, and no gate weakened |

## Goals and scope

1. **A `Subscription` root** with one field, `events(types: [String!]!, lastEventId: String): Event!`. `Event` is `{ id, type, data }`, the same object as a WebSocket frame. It is committed in the GraphQL snapshot as a change that is not breaking.
2. **GraphQL over SSE in the distinct connections mode** on `/api/graphql`, chosen by `Accept: text/event-stream`:
   - POST with a JSON body, for graphql-sse, urql and Relay;
   - GET with the request in the query string, for `EventSource`.
   - Every other request keeps today's JSON answers.
3. **The graphql-sse wire format.**
   - One `event: next` per execution result. Each subscription result also carries `id: <event id>`, so `EventSource` can resume.
   - `event: complete` with a bare `data:` line after a single-result operation or an error result.
   - Keep-alive comments.
4. **The same delivery as the existing streaming routes.** It reuses the hub, the window, the replay from Kafka, the cursor rules, the type rules, the subscriber cap and the readiness: at least once, ordered, resumable on any node, deduplicated by the client. The `Last-Event-ID` header wins over the `lastEventId` argument.
5. **Errors that match the protocol:**
   - invalid arguments, and an operation with more than one root field, are answered with `VALIDATION_FAILED` inside the accepted stream, and no hub subscriber is registered;
   - a node that is starting, at its capacity or shutting down answers the 503 problem before any stream.
6. **Clean ends.** A shutdown or a failed replay ends a subscription without `complete`, so the client resumes elsewhere. A client that closes its connection frees its slot.
7. **Browser access** through the existing `CORS_ALLOWED_ORIGINS` allow-list, for both the POST form and the `EventSource` form.
8. **Documentation:**
   - `docs/streaming-protocol.md` and `docs/requests.http` describe the GraphQL subscription and its reconnect procedure;
   - a decision record amends ADR 0007;
   - the architecture model and the load-test notes follow.
9. **The quality bar holds without exception:**
   - every local gate of `make ci` passes;
   - coverage is 100 % and no mutant survives in the changed code;
   - the K6 smoke scenario includes GraphQL subscribers;
   - no protected file changes beyond the three the maintainer approved (`.spectral.yaml`, `make/gates.mk`, `deny.toml`).
10. **Every route in the OpenAPI document and on the Swagger page** (maintainer decision of 2026-11-17): the streaming routes, both forms of the GraphQL endpoint, the liveness probe and the documentation routes join the REST operations, fully documented with their frames and problems; a test fails when a route is served without an entry; the OpenAPI gates pass on every operation and Schemathesis on every operation but the WebSocket handshake, whose exclusion the maintainer approved (`71aab46`), and a failure on a streaming operation is fixed at its root.
11. **No unmaintained crate in the production build** (M4): humantime leaves the two service crates, the durations keep their documented syntax, the envelope keeps its time rule, and the dependency policy passes with only the two dev-only exceptions the maintainer approved.

## Success criteria

| # | Measurable criterion | Traces to |
| --- | --- | --- |
| S1 | `.github/graphql-spec/spec` holds `type Subscription` with the `events` field returning `Event!`, and `type Event` with `id`, `type` and `data`. `make.validate_contract_snapshots` is clean, and `make.graphql_diff` reports no breaking change | AC1 |
| S2 | A POST of `subscription { events(types: ["a"]) { id type data } }` with `Accept: text/event-stream` answers `200`, `content-type: text/event-stream`. An event of type `a` written to the topic afterwards arrives as exactly one `event: next` whose `data` is `{"data":{"events":{"id":"<id>","type":"a","data":{<envelope>}}}}`, with `id: <id>`. An event of type `b` written in between does not arrive | AC2, M1 |
| S3 | The same subscription sent as `GET /api/graphql?query=...` with `Accept: text/event-stream` (the `EventSource` form) gives the same answers as S2 | AC2, AC8, M1 |
| S4 | Two subscriptions started with a last event id `c`, one carried by the argument and one by `Last-Event-ID`, each receive every retained event of their types with an id above `c`, in id order, before live events. The ids on one connection strictly increase, with no id twice. A header and an argument sent together resume from the header. The same holds on a second node of the same topic | AC3 |
| S5 | Five invalid subscriptions each answer `200 text/event-stream` with one `next` whose errors carry `extensions.code` `VALIDATION_FAILED`, then `event: complete` with a bare `data:`, then the end. The status `subscribers` does not change. The five are: an invalid type name, 101 distinct names, an empty list, a malformed cursor (argument or header), and two root fields, which break the direct-field rule (exactly one field selection written directly at the root) | AC4 |
| S6 | A node that is starting, at `MAX_SUBSCRIBERS` or shutting down answers a GraphQL subscription with `503 application/problem+json`, whose `detail` names the reason, and with no `text/event-stream` head. It logs `subscription rejected`. With `MAX_SUBSCRIBERS = n` filled by a mix of SSE, WebSocket and GraphQL subscribers, the next request of any of the three transports is refused, and `subscribers` counts the GraphQL ones | AC5 |
| S7 | While no event arrives, the stream writes a comment line right after the head and then one every `STREAM_KEEP_ALIVE`. A GraphQL client that stops reading delays neither the reader nor another subscriber's delivery, and its connection is closed by the send bound of every accepted socket | AC6 |
| S8 | A query (`{ eventTypes { name } }`) sent with `Accept: text/event-stream` answers exactly one `next` with `{"data":{"eventTypes":[...]}}`, then `complete`, then the end. Every request that does not negotiate SSE keeps its status and body; GET answers gain only `vary: accept`, and the 405 of other methods lists `GET, HEAD, POST`; the existing GraphQL, router and feature tests pass unchanged where they test such requests | AC7 |
| S9 | With `CORS_ALLOWED_ORIGINS` listing origin `O`, the preflight of the POST form allows `content-type` and `last-event-id`, and both forms answer with `Access-Control-Allow-Origin: O`. Without the variable, no answer carries `Access-Control-Allow-Origin` for `O` | AC8 |
| S10 | At shutdown, a GraphQL subscription stream ends after its queued events and without `complete`. The client then resumes with its last id on another node and misses no retained event | AC3, AC5, M1 |
| S11 | `docs/streaming-protocol.md` documents the request forms, the negotiation, the events, the errors and the 503 answers, the keep-alive, the end of a stream and the reconnect procedure for graphql-sse and `EventSource`. `docs/requests.http` has a POST and a GET subscription example and a query over SSE. ADR 0021 records the protocol choices. `make.docs_check` passes | AC11, M1 |
| S12 | Every gate of `make ci` that can run locally passes. `make.tests_with_coverage` reports 100 % lines and functions, and `make.mutation_tests_diff` reports no surviving mutant in the changed code. `make.smoke_load_tests` passes with GraphQL subscribers held within the existing `subscriptions` budgets. No protected file changed apart from the three approved changes (`.spectral.yaml`, `make/gates.mk`, `deny.toml`) | AC11 |
| S13 | `.github/openapi-spec/spec.yaml`, `GET /api/docs/openapi.json` and the Swagger page at `/api/docs` list exactly the eleven operations of the service: the three REST operations, `GET /api/events/stream`, `GET /api/events/ws`, `POST` and `GET /api/graphql`, `GET /ping` and the three documentation routes, each with its parameters, request body, every status code with its media type (`200 text/event-stream` with the event schema, `101 Switching Protocols` with the frames, the problems) and examples. A test fails when a route is served without an entry, and another when a documented operation is not served | AC9 |
| S14 | `make.validate_openapi_spec` reports no finding, `make.openapi_diff` reports only non-breaking changes, `make.validate_contract_snapshots` is clean, and `make.schemathesis_validate`, with the protected flags as amended by the approved `71aab46`, passes with ten of the eleven operations tested, the other streaming ones included; `GET /api/events/ws` is excluded only by that approved change and stays documented on the Swagger page; no other operation is excluded and no check is relaxed | AC10 |
| S15 | `make.check_security` passes; the manifests and `Cargo.lock` show `humantime` only as a dependency of `cucumber`, a dev-dependency of the end-to-end tests, so `cargo tree --workspace -e no-dev -i humantime` prints nothing; every duration value that `docs/environment.md` and the configuration tests name parses to the same `Duration`, with the same ranges and reasons; `occurredAt` accepts and refuses exactly the strings it does today; `docs/environment.md` names the duration syntax without humantime | M4 |

No capacity figure is set by this change. The design target of bundle 4 (10 000 subscribers per node, p99 fan-out below 100 ms at 100 events/s) is unchanged, and this change does not measure it.

## Out of scope

- A GraphQL WebSocket protocol (`graphql-ws`, `graphql-transport-ws`). The plain WebSocket route and the SSE route stay as they are.
- Subscriber authorization.
- The single connection mode of graphql-sse.
- JSON queries over GET.
- Running subscriptions from the development explorer.
- Incremental delivery (`@defer`, `@stream`, `multipart/mixed`).
- A subscription narrower than an event type.
- Per-client connection limits.
- A GraphQL mutation.

## Needs the maintainer's approval

Nothing beyond three protected changes that implementation and the advisories of 2026-11-20 made necessary, each already approved by the maintainer under `approved-config-change` and committed outside the stories (`research.md` section 6): the override of `operation-success-response` for the WebSocket operation in `.spectral.yaml`, the exclusion of that operation from Schemathesis in `make/gates.mk`, and the two advisory entries in `deny.toml` (M4). Otherwise:

- no other protected file changes, because the layer allow-lists, the CORS allow-list and the protected G20 contract test already fit the change (`research.md` sections 2.5 to 2.7 and 6); the replacement of humantime adds `jiff` only to the composition root, whose layer allows every crate;
- `make.graphql_diff` is expected to report the additions as not breaking, so no `approved-breaking-change` label is expected;
- the `.templatesyncignore` entries for the edited template files are not protected.
- the OpenAPI document only gains operations, so `make.openapi_diff` is expected to report no breaking change, and the protected G17, G19 and G21 tests keep their meaning (`research.md` section 2.8);
- no operation beyond the WebSocket handshake needs an exclusion from Schemathesis (`research.md` section 3.11); that one is the approved `71aab46`. Should another failure prove impossible to fix at its root, the story stops and asks the maintainer, naming the exact exclusion; an exclusion in the protected `make/gates.mk` needs `approved-config-change`.

## Assumptions

> Assumption: the brief adopts every decision of `research.md` section 8: the distinct connections mode only, the GraphQL endpoint chosen by `Accept`, GET for `EventSource`, the field and its object, the header winning over the argument, `VALIDATION_FAILED` inside the stream, the 503 before the stream, one root field per subscription, the wire format with `id:` lines, and no `complete` when a stream is cut. The brief restates them as goals and does not decide them again.

> Assumption: "no stream of events is opened" (AC4) is measured as "no hub subscriber is registered and no `next` event other than the one error result is written": the protocol requires validation errors to travel through an accepted SSE response, so the response itself is accepted.

> Assumption: "refuses a new subscription before it opens a stream" (AC5) is measured as an HTTP 503 problem answered instead of a `text/event-stream` head, which is the answer of the existing streaming routes and the one the graphql-sse client retries with backoff.

> Assumption: the graphql-sse client (exactly `Accept: text/event-stream`, POST) and the browser `EventSource` (GET) are the reference clients of the success criteria. urql's `Accept` headers are covered by the negotiation tests. Apollo needs a custom link and is documented, not tested.

> Assumption: success is measured on one node and on two nodes of one topic, as for the existing streaming routes. The K6 budgets are those of `tests/Load/config.json.dist`, and this brief sets no new number.

> Assumption: AC9's "every route the service serves" includes `/ping` and the three documentation routes besides the seven routes the issue names "at least", because the issue asks that a test fail for any route served without an entry.

> Assumption: AC10's fallback (exclude a streaming endpoint from Schemathesis when it cannot be tested) is not planned as a step: Schemathesis 4.33.1 bounds event streams and accepts a documented 101, and the one conflict found, its `Accept: */*` on `GET /api/graphql`, is fixed at its root by the GET rule of the negotiation (`prd.md` FR-2). An exclusion would be the maintainer's decision after an escalation; on 2026-11-17 that path excluded the WebSocket handshake, whose valid 101 Schemathesis reports as a failure (`71aab46`).

> Assumption: M4's "keeping the documented duration syntax" is measured on the values that `docs/environment.md`, `.env`, `.env.test` and the configuration tests name (`research.md` section 9.3): each parses to the same `Duration`, and the ranges and the two reasons stay. Units of days and longer, which no file of the repository uses, are not part of that syntax, nor are the other edges that the change moves (`prd.md` NFR-21); the bare `0` keeps its reading.
