# Research: GraphQL subscriptions over Server-Sent Events (issue #8)

Bundle `specs/8-graphql-subscriptions-over-sse/`, issue #8 as read on 2026-11-16 and again on 2026-11-17 (the maintainer's decision of 2026-11-17 added two criteria: every route in the OpenAPI document and on the Swagger page, and the OpenAPI gates and Schemathesis green), branch `claude/vilnacrm-notification-service-br7s47`, head `c36f708`. Pull request #9 also carries the finished cleanup of issue #7 (four commits, `07c2ff7` to `c36f708`). This bundle plans only issue #8. On 2026-11-20 the maintainer decided how to answer two RustSec advisories that made `make check-security` fail (M4); section 9 holds the facts, read on branch head `c4f83b3`, and the plan's answer, story 1.4. On 2026-11-21 the maintainer approved the length bounds of the GraphQL `query` and `operationName` that fixed a Schemathesis failure (M5); section 3.11 holds the Schemathesis facts.

Inputs of this stage:

- the issue body (requirements data);
- the three maintainer decisions:
  - M1: subscriptions travel over Server-Sent Events, following the GraphQL over SSE protocol of the graphql-sse project and the GraphQL over HTTP specification for the request formats, with every protocol choice recorded with its reason;
  - M2: the plan is made with BMAD through the rust-backend-sdlc plugin;
  - M3: only issue #8 is planned;
  - M4 (2026-11-20, the answer "Ignore both, replace ours"): RUSTSEC-2026-0542 (humantime) and RUSTSEC-2026-0543 (chrono) are ignored in the protected `deny.toml` by exactly the two approved entries, and one new story of this pull request replaces humantime in the two service crates (section 9);
  - M5 (2026-11-21, the answer "Approve the bound"): `query` holds at most 4 096 and `operationName` at most 128 characters (Unicode code points), documented as `maxLength` on the POST body and the GET parameters of `/api/graphql` and refused with the 422 validation problem on both transports and both forms, a recorded change of today's behaviour of the JSON transport (fix `56a8725`, section 3.11);
- the analyst's protocol research and codebase notes of 2026-11-16;
- the repository itself, read again for this file.

Tags used below:

- **[V]** verified on 2026-11-16 by reading the file and line named, or by command output.
- **[V src]** read on 2026-11-16 in a copy of the graphql-sse sources fetched into the session scratchpad for this stage (`PROTOCOL.md`, `src/handler.ts`, `src/common.ts`, `src/client.ts`, `src/parser.ts`, branch `master`).
- **[S]** taken from the analyst's protocol research, which fetched the source named on 2026-11-16; not re-read here.
- **[I]** an inference from the facts cited next to it.
- **[U]** not verified.

Source key (as in the analyst's research):

| Key | Source |
| --- | --- |
| P | graphql-sse `PROTOCOL.md` |
| H | graphql-sse `src/handler.ts` |
| C | graphql-sse `src/client.ts` |
| CM | graphql-sse `src/common.ts` |
| PR | graphql-sse `src/parser.ts` |
| R | the-guild.dev/graphql/sse/recipes |
| GOH | GraphQL over HTTP, draft (http-spec.graphql.org/draft) |
| W | WHATWG HTML, Server-sent events |
| F | WHATWG Fetch |
| MDN | MDN, EventSource |
| H2 | RFC 9113 |
| AP | Apollo Client, subscriptions |
| UD, UO, US | urql: subscriptions doc, `fetchOptions.ts`, `fetchSource.ts` |
| Y | GraphQL Yoga, subscriptions |
| AG | the registry copy of async-graphql 7.2.1, the version the workspace pins |
| UT, UA | the registry copies of utoipa 6.0.0 and utoipa-axum 0.3.0, the versions the lock pins |
| ST | the wheel of Schemathesis 4.33.1, the version `make/tool-versions.mk:38` pins, read on 2026-11-17 in the session scratchpad |

## 1. Decision and scope

- The issue asks for a `Subscription` root in the GraphQL schema. Its field delivers the events of the listed types. Subscription operations are served over SSE in the distinct connections mode of the graphql-sse protocol: one `next` event per execution result and a `complete` event at the end.
- The subscription must reuse the hub, the window, the resume and the replay from Kafka, the limits and the readiness rules of the existing streaming routes. It therefore keeps their guarantee: at least once, with deduplication by the client.
- The issue leaves these points to the plan: the field name and its arguments, the endpoint path, `GET` for `EventSource`, the mapping of `Last-Event-ID` and of the 503 states, and whether the single connection mode is needed. Section 8 decides each of them.
- Out of scope, from the issue: a GraphQL WebSocket protocol (`graphql-ws`, `graphql-transport-ws`); subscriber authorization. The plain WebSocket route stays as it is.
- Added by the maintainer on 2026-11-17: every route the service serves is in the OpenAPI document and on the Swagger page, fully documented; a test fails when a route is served without an entry; `make validate-openapi-spec`, `make openapi-diff` and `make schemathesis-validate` pass; a Schemathesis failure on a streaming endpoint is fixed at its root, and only when that is impossible is the endpoint excluded from Schemathesis, with the reason, staying on the Swagger page. Sections 2.8, 3.11 and 4.1 hold the facts.
- No new bounded context is needed. The change lives in `notification/event-stream` (application layer) and in the composition root `crates/service`. Both are live members of `[workspace].members` [V `Cargo.toml`], so the story 1 rule writes no scaffold story.

## 2. Current state of the touched contexts

### 2.1 Workspace

- The workspace has 12 members [V]:
  - `crates/shared/{domain,application,infrastructure}`;
  - `crates/internal/health-check/{domain,application,infrastructure}`;
  - `crates/notification/event-stream/{domain,application,infrastructure}`;
  - `crates/service`, `tests/e2e`, `tests/support` and `xtask`.
- The toolchain is 1.99.1, edition 2024 [V `rust-toolchain.toml`, `Cargo.toml:21`].
- The pinned crates are `async-graphql 7.2.1` with `default-features = false, features = ["uuid"]`, `axum 0.8.9` with `macros` and `ws`, `futures-util 0.3.34` with `alloc`, `tokio 1.54.1`, `serde_json 1.0.154` and `tower-http 0.7.1` [V `Cargo.toml:45-75`, `Cargo.lock`].
- An empty, untracked directory `crates/billing/invoices/` exists in the working tree. It is not a workspace member, and this change does not touch it [V].

### 2.2 How GraphQL is served today

- **Schema.**
  - `Query(EventStreamQuery)` is a `MergedObject`, and `ApiSchema = Schema<Query, EmptyMutation, EmptySubscription>` [V `crates/service/src/graphql/schema.rs:13-17`].
  - Depth 16 and complexity 1000 are enforced in `builder()` [V `:21-33`]. `sdl()` exports a sorted, byte-stable text [V `:37-47`].
- **Wiring.** `schema::builder().data(event_stream.clone()).finish()` puts the `EventStreamState` into the schema data [V `crates/service/src/bootstrap.rs:260`]. `StreamSettings::keep_alive` is part of that state [V `event-stream/application/src/http.rs:32-50`].
- **Handler.**
  - `execute(State<ApiSchema>, ApiJson<Map<String, Value>>)` requires a string `query`, else it answers the 422 problem "The request body is not a GraphQL request.". It then runs `schema.execute` and always answers 200 `application/json` [V `crates/service/src/graphql/handler.rs:27-44`].
  - The explorer page (GraphiQL `createFetcher`) is served only in `dev` [V `handler.rs:47-53`, `router.rs:35-38`].
- **A subscription sent today** is answered 200 with the error "Subscriptions are not supported on this transport." [V AG `schema.rs:501`].
- **The error helper.** `problem_to_error` turns a 422 problem into `VALIDATION_FAILED` with `violations`. It turns every other status into `INTERNAL_SERVER_ERROR` with no detail [V `crates/shared/application/src/graphql.rs:8-22`].
  - A resolver that wants `VALIDATION_FAILED` for a malformed cursor must therefore build a 422 problem with a violation; the cursor's own problem is a 400 [V `event-stream/application/src/http/dto.rs:306-313`].
- **The resolvers of the context** live in `event-stream/application/src/graphql.rs`. They read the state through `state(ctx)` [V `:85-88`].
  - The crate's module rule: only `http` and `graphql` may import `async_graphql`, `axum` or `utoipa` [V `lib.rs:8-10`].

### 2.3 What the event-stream application crate offers for reuse

- **Opening a subscription.**
  - `Subscriptions::open(SubscriptionRequest { types, cursor })` registers in the hub synchronously. It reads no broker and awaits nothing [V `http/subscription.rs:85-117`].
  - The refusal order is `Closed`, then `NotReady`, then `AtCapacity` [V `http/hub.rs:96-105`]. Dropping the `Subscriber` frees its slot and publishes the new count [V `hub.rs:388-398`].
  - A GraphQL subscriber that goes through `open` therefore counts against `MAX_SUBSCRIBERS` and appears in `subscribers` with no further code [I].
- **The refusal problem.** `From<HubError> for Problem` gives a 503 with the error text as `detail` and logs `subscription rejected` at `warn` [V `hub.rs:48-60`]. The three texts are "the node is starting", "the node is at its subscriber capacity" and "the node is shutting down" [V `hub.rs:36-46`].
- **The steps of a subscription.**
  - `Subscription::next` steps through the last page, the start of a replay, a window or Kafka page and the live queue. An idle step spends cooperative budget [V `subscription.rs:203-237`].
  - It ends with `None` on hub close or replay failure. `failure()` then holds the error chain of a failed replay [V `:120-127`, `:290-294`].
  - The ids on one connection strictly increase (bundle 4 FR-23) [V `docs/streaming-protocol.md:100`].
- **Slow clients.**
  - `Hub::deliver` uses `try_send`. A full queue makes the subscriber fall behind; it re-arms and replays on its own connection [V `hub.rs:199-220`].
  - Every accepted socket gets `TCP_USER_TIMEOUT = STREAM_KEEP_ALIVE` [V `crates/service/src/command/serve.rs:111-125`].
  - A GraphQL stream built on the same `Subscription` inherits both [I].
- **The SSE route.**
  - `stream` checks the types, then the cursor, then calls `open`, then answers `Sse::new(unfold(subscription, next_event)).keep_alive(..)` with `x-accel-buffering: no` [V `http/handler.rs:109-126`].
  - `next_event` ends without a final frame, and logs `replay failed` with transport `sse` when a replay failed [V `:175-185`].
- **Shutdown.** `hub.close()` ends every queue after its queued frames. The node then waits for `drained()` for at most `STREAM_DRAIN_TIMEOUT` (2 s) [V `crates/service/src/background.rs:19`, `:118-119`; `hub.rs:224-232`].
- **Validation.**
  - `parse_names` takes an iterator of names. It answers `TypesError` with the violation codes `required`, `length`, `pattern` and `max_items`; it is `pub(crate)` [V `dto.rs:206-268`].
  - `parse_cursor` reads the `Last-Event-ID` header first and the parameter only when the header is absent [V `dto.rs:288-303`].
- **The frame.**
  - The hub encodes one `Frame { sequence, event_type, sse, json }` per event and shares it. `json` is `{"id":"<seq>","type":"<type>","data":<event object>}` [V `http/frame.rs:10-49`].
  - That is the shape the issue asks the GraphQL field to return. The parsed event object is not stored separately [V].

### 2.4 Router, middleware and CORS

- **The route.** `/api/graphql` is `post(execute)`, plus `.get(explorer)` in `dev`. A `GET` outside `dev` is the 405 problem with `Allow: POST` [V `router.rs:35-38`, `router/tests.rs:297-320`, `features/graphql.feature:6-9`].
- **The layers**, outermost first: request id ×3, security headers, trace, CORS (only when origins are allowed), WebSocket origin check, panic catcher, request budget, body limit [V `router.rs:39-57`].
- **The request budget** wraps only `next.run`, the time to the response head [V `shared/application/src/http/middleware.rs:13-21`; `docs/architecture.md:37`]. An operation run inside a streamed body would escape it [I].
- **CORS** allows `GET` and `POST`, the headers `content-type` and `last-event-id`, and a max age of 600 s [V `crates/service/src/http/origin.rs:26-38`].
  - With no allowed origin, the router has no CORS layer at all [V `origin.rs:23-31`].
  - The layer sets no `allow_credentials`, so credentialed cross-origin reads are not offered [V].
- **The access log** records `url.path` only, not the query string [V `crates/service/src/http/trace.rs:33-38`]. A GraphQL document carried in a `GET` query string is therefore not written to the log [I].

### 2.5 Tests that pin today's behaviour (oracles, none protected)

| Test | What it pins | Effect of this change |
| --- | --- | --- |
| `crates/service/src/graphql/schema/tests.rs:65-82` | the SDL holds no `type Subscription`, `node`, `Connection`, `update`, `delete` or `input ` (descriptions included) | `type Subscription` becomes allowed; the other words stay forbidden, so new descriptions avoid them |
| `schema/tests.rs:85-100` | the first `type` block is `EventStreamStatus` with twelve single-line fields | a type named `Event` sorts first; the oracle names it instead |
| `crates/service/src/http/router/tests.rs:402-408` | handler parameters of `crates/service` are only `State<ApiSchema>`, `ApiPath<`, `ApiJson<`, `ValidatedQuery<`, `ValidatedJson<` | a handler that reads `Accept` and `Last-Event-ID` needs `HeaderMap`, the recorded streaming exception (`event-stream/application/src/http/handler/tests.rs:1545-1550`); the GET handlers also need `RawQuery` |
| `router/tests.rs:412-419` | the set of handler files | changes only when a new file holds a `pub async fn` |
| `schema/tests.rs:8`, `:60-62` (`EXPECTED_SDL`, `the_sdl_is_the_documented_schema`) | the full SDL of today's schema | becomes the full SDL of FR-1 |
| `router/tests.rs:690-695` (`has_cors_headers`) | any `vary` header counts as a CORS header | narrowed to an `access-control-*` header or a `vary` value `origin`, because every GET answer of `/api/graphql` gains `vary: accept` |
| `router/tests.rs:297-320`, `features/graphql.feature:6-9` | `GET /api/graphql` outside `dev` is 405 with `Allow: POST` | kept: section 8 keeps every answer that does not negotiate SSE |

- The protected Bats test G20 removes the first single-line field of the first `type` block of the snapshot and expects `graphql-diff` to fail [V `tests/CLI/bats/make_contracts.bats:83-90`, `:209-217`].
  - With a first type `Event { data: JSON!, id: String!, type: String! }` it removes `data: JSON!`. That is still a field removal, so G20 keeps its meaning without a change to the protected file [I].
- No Bats test names `/api/graphql` [V `grep`].

### 2.6 Contracts, end-to-end, integration, load and documentation

- **The snapshot.** `.github/graphql-spec/spec` holds `EventStreamStatus`, `EventType`, `Query` and `schema { query: Query }` [V]. It is regenerated by `make.generate_graphql_spec` and checked by `make.validate_contract_snapshots` [V `Makefile:315-319`, `make/gates.mk:33-34`].
- **The breaking-change gate.** `make.graphql_diff` runs the protected `scripts/graphql-diff.sh` (graphql-inspector 7.0.2) [V `Makefile:329-331`].
  - The analyst read core 8.0.3: a subscription root added where none existed is `NonBreaking`, and new types and scalars are non-breaking [S]. No `approved-breaking-change` label is expected [I].
- **End-to-end tests.**
  - `tests/e2e/world.rs:695` `open_sse` is GET-only, and `SseEvent::parse` panics on a field other than `id`, `event` or `data` [V `world.rs:133-151`].
  - The features are `event_stream.feature`, `event_stream_websocket.feature` and `graphql.feature` [V].
- **Integration tests.** `crates/service/tests/integration/client.rs:99-131` is a raw HTTP/1.1 SSE client, GET only [V]. Two-node and served-binary tests exist from bundle 4 (`cluster.rs`, `serve_stream.rs`) [S].
- **Load test.** `tests/Load/scripts/subscriptions.js:122-125` holds SSE subscribers with `http.asyncRequest('GET', ...)`, tagged `held_subscription` [V].
  - The `subscriptions` budgets of `tests/Load/config.json.dist` are not protected; only the four `health` thresholds and `thresholdsBuilder.js` are [V `protected-config.toml`].
  - `smoke-load-tests` is a delivery target of `make ci` [V `Makefile:93`].
- **Documentation.**
  - `docs/streaming-protocol.md` documents the answers before a stream opens, SSE, WebSocket, browser access and reconnecting [V headings].
  - `docs/requests.http` has the GraphQL query examples [V `:43-57`].
  - ADR 0007 says "with no subscription" [V `docs/adr/0007-api-contracts.md:15`]. The ADR index already records one record superseded by a later one (0005 by 0020) [V `docs/adr/README.md:11`].
  - `workspace.dsl:15` describes the service container's routes [V].

### 2.7 Allow-lists, protected files and template sync

- **Allow-lists.** The application layer may use `async-graphql`, `axum`, `futures-util`, `serde_json`, `tokio` and the rest of its list. It may not use `tokio-stream` or `async-stream`. The composition root allows `*` [V `architecture.toml:22-52`, `:76-80`].
  - Everything this change needs is in `async-graphql` (`Subscription`, `MergedSubscription`, `Json`, `execute_stream`, extensions), `futures-util` and `tokio` [V AG `lib.rs`, `schema.rs:573-660`, `types/json.rs:79-96`].
  - No protected file needs to change for these crates [I]. The protected changes approved after this research are listed in section 6.
- **Template sync.** These template files would be edited and are not yet in `.templatesyncignore` [V `git log --diff-filter=A`: added by "Initial commit"]:
  - `crates/service/src/graphql/handler.rs`;
  - `crates/service/src/graphql.rs`, when a module is added.
- Already listed or service-owned [V `.templatesyncignore`, `git log`]:
  - `schema.rs`, `schema/`, `handler/tests.rs`, `router.rs`, `router/tests.rs`, `origin.rs`, `bootstrap.rs`;
  - `features/graphql.feature`, `tests/e2e/`, `docs/requests.http`, `docs/adr/`, `docs/architecture.md`, `docs/load-testing.md`, `workspace.dsl`, `.github/graphql-spec/`, `tests/Load/config.json.dist`, `crates/notification/`, `crates/service/tests/integration/main.rs`;
  - the service-owned `docs/streaming-protocol.md`, `tests/Load/scripts/subscriptions.js`, `crates/service/tests/integration/client.rs` and the event-stream features.
- `crates/shared/application/src/graphql.rs` is a template file too. Section 8 keeps it unchanged [I].

### 2.8 The OpenAPI document, the Swagger page and their gates (added on 2026-11-17)

- **The document today** holds three operations: `GET /api/event-types`, `GET /api/events/status` and `GET /api/health` [V `.github/openapi-spec/spec.yaml:16-112`]. `openapi::api_router()` nests the two context routers under `/api` and `document()` is `api_router().into_openapi()` [V `crates/service/src/openapi.rs:46-68`]. `API_VERSION` is `2.0.0`, and its comment asks for a minor bump when operations are added [V `openapi.rs:9-13`].
- **Routes served outside the document:**
  - the event-stream router registers the two streaming routes with a plain `.route(..)` [V `crates/notification/event-stream/application/src/http.rs:65-66`], and its comment and the handler module say they "are not operations of the `OpenAPI` document" [V `http.rs:55-56`, `http/handler.rs:3-6`];
  - `assemble` registers `/api/graphql`, `/ping`, `/api/docs`, `/api/docs/openapi.json` and `/api/docs/{*file}` with `.route(..)` [V `crates/service/src/http/router.rs:59-63`];
  - ADR 0006 says `/ping` is "registered outside `/api` and outside the OpenAPI router" [V `docs/adr/0006-health-and-liveness.md:11`], and `docs/architecture.md:7` says the streaming routes are "outside the OpenAPI document" [V];
  - bundle 4 kept the streaming routes out because a never-ending response "would hang the Schemathesis gate" (bundle 4 `prd.md`, assumption 29) [V]. Section 3.11 shows that the pinned Schemathesis no longer hangs on one.
- **The Swagger page.** `GET /api/docs` serves the Swagger UI page; `GET /api/docs/openapi.json` serves `openapi::document()`; `GET /api/docs/{*file}` serves an embedded Swagger UI file or the 404 problem, and the 400 problem of `ApiPath` for an undecodable sub-path [V `crates/service/src/http/docs.rs:23-50`]. `features/api_docs.feature` checks the page and that the JSON names `/api/event-types` [V].
- **Oracles that pin the scope** (ordinary test files): `documents_exactly_the_three_operations`, `leaves_the_liveness_route_and_the_docs_routes_out`, `defines_exactly_the_five_schemas` and `carries_the_service_identity_and_the_contract_version` [V `crates/service/src/openapi/tests.rs:65-130`, `:155-171`].
- **The gates.**
  - `make.validate_openapi_spec` lints the snapshot with Spectral and the protected `.spectral.yaml`: every operation needs a summary, a description, tags, a 2xx or `default` response and a 4xx or `default` response [V `.spectral.yaml:31-200`]. A `101` alone would not satisfy the 2xx rule; the `default` problem response does.
  - `make.openapi_diff` runs the protected `scripts/openapi-diff.sh` (oasdiff) against `base_ref` [V `Makefile:325-327`]; added paths are not breaking [I].
  - `make.schemathesis_validate` runs `schemathesis run /data/spec.yaml --url $(SCHEMATHESIS_URL) $(SCHEMATHESIS_FLAGS)` against the production image in the load-test stack [V `Makefile:82`, `:333-338`]. The flags `--checks all --phases=examples,coverage` live in the protected `make/gates.mk:35` [V]. The load-test stack runs `APP_ENV: test` [V `docker-compose.load_test.override.yml:5`] and its publisher writes `load.test` events [V `docs/load-testing.md:147`].
- **Protected Bats tests of these gates** [V `tests/CLI/bats/make_contracts.bats`]:
  - G17 removes the first line `      description: ` of the snapshot (the first operation's description) and expects Spectral to fail [V `:45-48`];
  - G19 removes the required `title` of `Problem` and expects oasdiff's `response-required-property-removed` [V `:50-58`, `:180-196`];
  - G21 plants `/api/planted_undocumented` and expects `schemathesis-validate` to fail with that name and to leave no project behind; it runs the whole Schemathesis run [V `:62-78`, `:264-277`].
  - With paths sorted, the first operation becomes `GET /api/docs`; its operation-level `description` is still the first such line, so G17 keeps its meaning [I]. `Problem` stays referenced by every problem response, so G19 keeps its meaning [I].
- **`scripts/check-docs.sh`** (protected) accepts a cited route by the first segments of the snapshot's paths, the `.route("/..` literals of `router.rs` and `ping` [V `:143-153`]. With every route in the snapshot, `api` and `ping` stay accepted even when `router.rs` holds no `.route(` call [I].
- **Template sync.** `crates/service/src/openapi.rs`, `openapi/`, `router.rs`, `router/tests.rs` and `.github/openapi-spec/` are listed in `.templatesyncignore`; `crates/service/src/http/ping.rs`, `crates/service/src/http/docs.rs` and their test directories `ping/` and `docs/` are template files that it does not list [V].
- **utoipa** 6.0.0 and utoipa-axum 0.3.0 are workspace dependencies of `crates/service` and of the event-stream application crate [V `Cargo.toml:78-79`, the two manifests], and the application layer's allow-list already holds them [V `architecture.toml`].

## 3. Protocol findings

### 3.1 The two modes of graphql-sse

- **Distinct connections mode.** Each operation is its own GraphQL over HTTP request, answered as `text/event-stream`. A streaming operation ends when the client closes the connection [V src P:11-20].
  - It is the client's default (`singleConnection = false`) [V src C:286]. The recipes say it "should suffice for most of the use-cases" [S R]. Yoga serves it out of the box [S Y].
- **Single connection mode.** A `PUT` reservation answers 201 with a token, sent in `X-GraphQL-Event-Stream-Token` or `?token=`. One SSE connection holds it. Each operation is a POST answered 202, and `DELETE ?operationId=` stops one [V src P:60-120].
  - The reference handler keeps reservations in process memory, answers 409 to a second stream on the same token, and 404 without a token outside SSE [V src H:778-810].
- **Fit with this service.** The single mode needs the PUT, the stream, every POST and the DELETE to reach the node that holds the reservation, so it needs sticky sessions or shared state. Nodes here are stateless behind a load balancer (ADR 0014). The distinct mode keeps each request self-contained [I].

### 3.2 Request formats and content negotiation

- **Parameters.** `query`, `operationName`, `variables` and `extensions` [S GOH §4.1]. POST with an `application/json` body is required, and GET is optional [S GOH §4.3, §4.4].
  - Over GET, `variables` and `extensions` are JSON strings, and a mutation is a 4xx, 405 with `Allow` recommended [S GOH §4.3]. GraphQL over HTTP leaves subscriptions out of its scope [S GOH §1].
- **The reference handler**:
  - parses GET from the query string;
  - answers 415 to a POST without `application/json`;
  - answers 400 to a missing or invalid `query`, `variables` or `extensions` [V src H:1055-1074].
- **Accept, per the specification.** The server uses the highest-priority type it supports; otherwise it answers 406 or ignores `Accept` [S GOH §4.2, §5.1].
- **Accept, in practice:**
  - The reference streams only when `accept === 'text/event-stream'` exactly, under a "TODO: make accept detection more resilient" [V src H:736-741].
  - The graphql-sse client POSTs JSON with exactly `accept: text/event-stream` [S C].
  - urql's `fetchSubscriptions` sends `text/event-stream, multipart/mixed` for subscriptions. For queries it sends `application/graphql-response+json, application/graphql+json, application/json, text/event-stream, multipart/mixed` [S UO].
  - An exact match therefore breaks urql, and "contains" would stream urql's queries. `Accept` must be parsed as a list of media ranges [I].
- **EventSource** sends only GET, cannot add headers, may set `Accept: text/event-stream` itself, and uses cache mode no-store [S W §9.2.2]. The recipe passes `query` in the URL [S R].

### 3.3 Events on the wire

- **The two events.** `next` carries an ExecutionResult. `complete` follows after all results [V src P:28-52].
- **The reference's bytes.** It prints `event: <name>\ndata:`, then a space and the JSON when data is not null, then `\n\n` [V src CM:70-80]. So `complete` is `event: complete\ndata:\n\n`.
- **Why `complete` needs a `data:` line.** The protocol requires an empty `data:` field, or an `EventSource` listener does not fire [V src P:52]. The reason: an event whose data buffer is empty is not dispatched, and a `data:` line appends a line feed to it [S W §9.2.6].
- **axum cannot produce that line.** `Event::data("")` writes no `data` line, because `write_buf` returns early on an empty buffer [V axum `src/response/sse.rs:441-444`]. A byte-level body is needed [I].
  - axum adds `content-type: text/event-stream` and `cache-control: no-cache` only for its own `Sse` [V `sse.rs:97-98`].
- **Response headers.** The Content-Type MUST be `text/event-stream` [V src P:15].
  - The reference adds `charset=utf-8`, `cache-control: no-cache`, `connection: keep-alive` and `content-encoding: none` [V src H:770-773].
  - A `Connection` header MUST NOT be generated on HTTP/2 [S H2 §8.2.2].
- **Client parsers.**
  - The graphql-sse parser reads only `event` and `data` and ignores `id` and `retry` [V src PR:110-113].
  - Any event name other than `next` or `complete` is fatal ("Invalid stream event") [V src CM:65]. The GraphQL stream must therefore never reuse the event-type names of the existing SSE route [I].
  - urql reads the first `data:` line of each block and stops at `hasNext: false` [S US], so each JSON value must sit on one line [I]. A `complete` with `data: null` would reach urql as a result [I].
- **Queries over SSE.** One result becomes one `next`, then `complete` [S H].
  - `Schema::execute_stream` yields one item for an operation that is not a subscription. It also yields parse, validation and limit errors as the only item [V AG `schema.rs:586-626`].

### 3.4 Errors and status codes

- **The protocol rule.** Validation steps that run before execution MUST report their errors through an accepted SSE connection, as `next` events. A 400 makes the user agent fail the connection and, for `EventSource`, hides the reason [V src P:16-18].
- **The reference.**
  - It answers a GraphQL validation error with 200 and `next {errors}` when `Accept` is SSE [V src H:689-699].
  - It answers 400, 405 or 415 before any stream for a malformed request [V src H].
- **How clients react.**
  - graphql-sse treats any non-2xx as a network error and retries. It makes 5 attempts with randomised exponential backoff of 1 s × 2^n plus 300 ms to 3 s [V src C:304-316].
  - `EventSource` fails the connection on any status other than 200 or on a wrong type, and does not reconnect [S W §9.2.2].
- **GraphQL over HTTP §5.4 statuses** [S GOH]:
  - 400: unparsable JSON or document.
  - 422: a request that is not well formed, a validation failure, an ambiguous operation, or variables that cannot be coerced.
  - 405, 406, 413, 414 and 415: as named in §5.4.
  - 503: maintenance or load shedding.

### 3.5 Keep-alive

- The protocol is silent on keep-alive.
- The reference writes `:\n\n` as soon as it accepts a stream, because "some browsers (like Firefox and Safari) dont accept the header flush". It then repeats it every 12 s [V src H:516-519].
- WHATWG suggests a comment about every 15 s for legacy proxies. Both parsers ignore comments [S W §9.2.6-7, V src PR].

### 3.6 Event ids and Last-Event-ID

- **The protocol defines neither.** The reference's types mark `id` as possibly used "in future releases for connection recovery" [S CM].
- **EventSource** keeps the last `id:`. It sends it as `Last-Event-ID` on every reconnection, and it reconnects even after a normal end of the body [S W §9.2.2-6].
- **The graphql-sse client** re-sends the same POST body on a retry and never adds `Last-Event-ID` itself. Its `headers` and `url` functions are evaluated again on every attempt, so an application can add the header [S C].
- **Consequence.** An `id:` line on each subscription `next` is ignored by graphql-sse and urql and lets `EventSource` resume [I]. A body re-sent by graphql-sse still carries the first cursor, so the header must win over the argument [I].

### 3.7 How a stream ends

- **Server side.** `complete` follows the last result [V src P:30].
- **Client side.** The client ends a subscription by closing the connection [V src P:20]. An `EventSource` must call `close()` on `complete`, or it reconnects [S R, W].
- **The graphql-sse client.**
  - If the connection closes without `complete` while results are expected, it raises "Connection closed while having active streams" and retries.
  - After `complete` it does not reconnect, and a received result resets its retry count [S C].
- **Consequence.** A stream cut by a node that shuts down should end without `complete`, so the client resumes on another node. `complete` is sent only when the operation has really finished [I].

### 3.8 Connection limits

- Over HTTP/1.x a browser allows 6 connections per origin, shared across tabs ("Won't fix" in Chrome and Firefox) [S MDN, P].
- Over HTTP/2 the limit is negotiated: at least 100 is recommended [S H2 §6.5.2], and Yoga cites about 100 [S Y].
- WHATWG suggests a shared worker or one connection per page [S W §9.2.7].
- Browsers negotiate HTTP/2 only over TLS [U].

### 3.9 CORS

- **EventSource** sends a CORS-mode request. The `Accept` and `Last-Event-ID` headers it sets itself do not trigger a preflight [S W §9.2.2, F §2.2.5].
- **The response check.** `Access-Control-Allow-Origin` must equal the requesting origin, or be `*` without credentials [S F §4.10].
- **Clients built on fetch** POST `Content-Type: application/json`, which preflights. The server must allow `content-type`, and `last-event-id` when the application adds it. `Accept: text/event-stream` is safelisted [S F §2.2.2].
  - The graphql-sse client defaults to `credentials: 'same-origin'` [V src C:319].
- **This service.** The CORS layer of section 2.4 already allows all of this [I].

### 3.10 Clients

- Apollo Client has no built-in SSE transport. It needs a custom link around graphql-sse [S AP, R].
- urql works with `subscriptionExchange` plus graphql-sse, or with `fetchSubscriptions: true` [S R, UD].
- A Relay recipe exists [S R].

### 3.11 Schemathesis 4.33.1 against streaming operations

- **Event streams end.** A response whose `Content-Type` is `text/event-stream` is read until the stream ends, 20 events arrive or the request timeout passes, 10 s by default; the response is then marked "cut short" and checked like any other [V ST `core/transport.py:56-90`, `transport/requests.py:320-335`]. A silent stream ends at the 10 s read timeout [V ST `core/transport.py:84-90`]. So a documented SSE operation no longer hangs the run that bundle 4 feared [I].
- **Each event is validated** against the schema of the `text/event-stream` media type, as an object `{data, event, id?, retry?}` (the OpenAPI 3.2 convention), with `contentMediaType` and `contentSchema` honoured for `data` [V ST `core/deserialization.py:223-256`, `:305-308`, `specs/openapi/validation.py:140-163`, `specs/openapi/content_keywords.py:30-90`]. A plain `type: string` schema would therefore fail every event [I]; the document must describe one event.
- **Default expectations of the checks** [V ST `config/_checks.py:13-29`]:
  - `not_a_server_error`: 2xx, 3xx, 4xx;
  - `positive_data_acceptance`: 2xx, 3xx, 401, 403, 404, 409, 429, 5xx, so a 405, 400 or 422 to schema-valid data fails;
  - `negative_data_rejection`: 400, 401, 403, 404, 405, 406, 409, 415, 422, 428, 429, 5xx, so a schema that refuses a value the service accepts fails;
  - `missing_required_header`: 400, 401, 403, 406, 415, 422;
  - `unsupported_method` sends undocumented methods in the coverage phase and expects 405 with an `Allow` header [V ST `specs/openapi/checks.py:1148-1186`].
- **`Accept`.** Its run records `Accept` as always `*/*` [V ST `reporting/ndjson.py:70`]. A `GET /api/graphql` with schema-valid parameters would get today's 405 and fail `positive_data_acceptance` [I]. The GET rule of `prd.md` FR-2 answers `*/*` on GET with the event stream, which removes that failure at its root [I].
- **A 101 response** has no body for the HTTP client [I, Python `http.client` reads no body for 1xx]; a connection that the server switched to WebSocket and that the client reuses is dropped, and Schemathesis sends that request once more on a fresh connection [V ST `transport/requests.py:297-315`]. A documented `101` passes `status_code_conformance` [I].
- **Oversized body strings** (added on 2026-11-21). In the coverage phase, a string in a request body whose schema has no `maxLength`, `pattern`, `format`, `enum` or `const` gets a positive case of `OVERSIZED_STRING_LENGTH = 2**16 + 1`, 65 537 characters, described "String longer than common implicit limits"; parameters in the query string or a header do not get it [V ST `specs/openapi/coverage/_schema.py:3248-3249`, `:3387-3405`]. A `POST /api/graphql` whose `query` holds it exceeds the default `HTTP_MAX_BODY_BYTES` of 65 536 and gets the 413, which `positive_data_acceptance` does not accept: the CI run on `9bf9de6` failed with ST202 on that operation, and `operationName` would fail the same way [V CI, `9bf9de6`]. With a `maxLength`, the case is not generated, and the coverage phase adds the negative case of `maxLength + 1` characters, "String larger than maxLength" [V ST `_schema.py:2402-2427`], which `negative_data_rejection` accepts as 400 or 422 and not as 413 [V ST `config/_checks.py:14-28`]. So the bound must be refused by a check inside the service, below the size limit: the fix `56a8725` gives `query` a `maxLength` of 4 096 and `operationName` one of 128 and refuses a longer member with the 422 problem; the exact gate command then passed against a node of the fixed tree (230 test cases, 0 failures), and CI on `99da8cb` passed all 49 checks, Schemathesis included [V CI]. The maintainer approved the bound, which also refuses a JSON-transport request that ran before, as M5.
- **Parameters with JSON content.** Schemathesis reads `contentMediaType` and `contentSchema` [V ST `content_keywords.py`], and utoipa 6.0.0 can write `content` for a parameter and `contentMediaType` and `contentSchema` for a string schema [V UT `src/openapi/path.rs:810-845`, `src/openapi/schema.rs:1352-1361`], so the JSON-encoded `variables` and `extensions` of the GET form can be described exactly [I].
- **Cost.** Each schema-valid streaming case that gets no event costs up to 10 s; examples that subscribe to `load.test` reach 20 events quickly [I]. The CI job "Schemathesis" took 4 min 34 s on 2026-11-11 [V `docs/operations.md:399`].

## 4. async-graphql 7.2.1 facts

- **No feature needed.** The `Subscription` and `MergedSubscription` derives, `execute_stream` and `Json` are not behind a feature [S AG `lib.rs:280`, `:292`]. `Schema::build(q, m, s)` takes the subscription root [S AG `schema.rs:358-363`].
- **`execute_stream` is lazy** [V AG `schema.rs:573-660`]:
  - preparation errors become one `Response::from_errors`;
  - an operation that is not a subscription yields one result;
  - a subscription merges one stream per root field with `select_all`.
- **The derive runs the resolver lazily.** It runs only when its stream is first polled (`stream::once(..).try_flatten`) [V async-graphql-derive `subscription.rs:385-386`].
  - A resolver error becomes one error item, and the stream ends [S].
  - Every item is resolved against the client's selection set [S].
- **The HTTP status depends on the first poll** [I]. A refusal or an argument error is visible only after the stream is first polled. A handler that chooses the status, or that opens no SSE response on a refusal, must drive the operation before it writes the head, under the request budget.
  - "Pending at the first poll" is not a safe signal: `Subscription::next` yields cooperatively on idle steps [V `subscription.rs:229-233`].
- **No single-root-field rule.** async-graphql has no "single root field" rule: `validation/rules/` has 23 rules and none for subscriptions [V `ls`].
  - `subscription { a: events(..) b: events(..) }` would open two hub subscribers in one request [I].
  - `Extension::parse_query` exists as a hook that sees the parsed document before validation and execution [V AG `extensions/mod.rs:410`, `:523`].
- **Response serialisation.** `Response` always serialises `data` (null on a request error) and skips empty `errors` and `extensions` [V AG `response.rs:8-30`]. That is an ExecutionResult [I].
- **The JSON scalar.** `Json<T>` is the scalar `JSON`, described "A scalar that can represent any JSON value." [V AG `types/json.rs:79-96`]. The description holds no word the schema test refuses [V].
- **Avoid `async_graphql::http::parse_query_string`.** It reads `operation_name` in snake case [S AG `http/mod.rs:40-70`]. A camel-case `ValidatedQuery` struct is needed for GET [I].
- **No ready-made transport.** `async-graphql-axum` is not in the registry and serves only WebSocket subscriptions [S].

### 4.1 utoipa 6.0.0 and utoipa-axum 0.3.0 facts

- `OpenApiRouter::routes(routes!(..))` registers each annotated handler in the axum router at the annotated path and adds the same path item to the document; the path string is used verbatim for both [V UA `src/router.rs:215-249`]. A plain `OpenApiRouter::route(..)` adds nothing to the document [V UA `src/router.rs:251`].
- So the catch-all `/api/docs/{*file}` cannot be registered through `routes!` without writing `{*file}` into the document, which is not an OpenAPI path template [I].
- Any status code, including `101`, can be documented, with response headers and examples per media type [I, `utoipa-gen` `responses(..)` syntax as the existing handlers use it].

## 5. The surface the change touches

| Path | Change | Sync |
| --- | --- | --- |
| `crates/notification/event-stream/application/src/graphql.rs` (+ `tests.rs`) | the subscription resolver and its `Event` object; argument checks through `parse_names` and a cursor check that answers `VALIDATION_FAILED`; `Subscriptions::open` | listed (`crates/notification/`) |
| `event-stream/application/src/http/{dto,frame,subscription}.rs` | crate-internal reuse: the cursor check of one carrier, the event object of a frame, the `replay failed` log with its transport label | listed |
| `crates/service/src/graphql/schema.rs` (+ `schema/tests.rs`) | the `Subscription` root, the schema type, the single-root-field check; new SDL oracles | listed |
| `crates/service/src/graphql/handler.rs` (+ `handler/tests.rs`) | `Accept` negotiation, the GET form, the SSE transport (head, `next`, `complete`, `id:`, keep-alive, end) | add `handler.rs` to `.templatesyncignore` |
| `crates/service/src/graphql.rs` | a module declaration if the transport gets its own file | add to `.templatesyncignore` if edited |
| `crates/service/src/http/router.rs` (+ `router/tests.rs`) | GET on `/api/graphql` in every mode; `HeaderMap` in the allowed extractors; CORS tests of the GraphQL stream | listed |
| `.github/graphql-spec/spec` | regenerated with `Subscription`, `Event` and `JSON` | listed |
| `features/graphql.feature` or a new GraphQL subscription feature; `tests/e2e/` | POST and GET SSE helpers; scenarios for every criterion | listed |
| `crates/service/tests/integration/` | real-broker subscription and resume through GraphQL over SSE | `main.rs` listed; `client.rs` service-owned |
| `tests/Load/scripts/subscriptions.js`, `docs/load-testing.md` | held GraphQL SSE subscribers within the existing `subscriptions` budgets | service-owned / listed |
| `docs/streaming-protocol.md`, `docs/requests.http`, `docs/architecture.md`, `docs/adr/0021-*.md`, `docs/adr/README.md`, `workspace.dsl` | the GraphQL subscription, its answers and its reconnect procedure; the decision record | service-owned / listed |
| `event-stream/application/src/http.rs`, `http/handler.rs`, `http/dto.rs` | the streaming routes registered through `routes!`, their `#[utoipa::path]` annotations and the `IntoParams` of their parameters | listed |
| `crates/service/src/openapi.rs` (+ `openapi/tests.rs`), `http/router.rs`, `graphql/handler.rs`, `http/ping.rs`, `http/docs.rs` | the three routers merged into one document, the annotations of the GraphQL, liveness and documentation handlers, the completeness oracles | `openapi*`, `router.rs` listed; add `ping.rs`, `ping/`, `docs.rs` and `docs/` |
| `.github/openapi-spec/spec.yaml`, `docs/adr/0022-*.md`, `features/api_docs.feature` | the regenerated document with eleven operations; the decision record; a scenario that finds every operation | listed |
| `Cargo.toml`, `Cargo.lock`, `crates/service/Cargo.toml`, `crates/notification/event-stream/application/Cargo.toml`, `crates/service/src/config.rs` (+ `config/tests.rs`, `config/tests/kafka.rs`), `event-stream/application/src/envelope.rs` (+ `envelope/tests.rs`), `docs/environment.md` | humantime out of the production build: jiff parses the durations, the envelope checks its time without a crate (section 9, story 1.4) | listed (`Cargo.toml`, `Cargo.lock`, `crates/service/Cargo.toml`, `config.rs`, `config/`, `crates/notification/`, `docs/environment.md`) |

## 6. Protected files

- As researched on 2026-11-16 and 2026-11-17, no protected file changes:
  - the allow-lists already cover the crates (section 2.7);
  - the CORS allow-list already covers the headers (section 2.4);
  - G20 keeps its meaning with the new first type (section 2.5);
  - the `subscriptions` load budgets are not protected (section 2.6);
  - the OpenAPI document only gains operations, Spectral's protected rules are met with a `default` problem response on every operation, and G17, G19 and G21 keep their meaning (section 2.8);
  - Schemathesis can test every operation with its protected flags unchanged (section 3.11), so no exclusion in `make/gates.mk` is planned.
- Amended on 2026-11-20: four protected changes proved necessary after this research. Each was approved by the maintainer under `approved-config-change`, committed outside the stories, and is recorded in `architecture.md` "Protected Changes":
  - the override of `operation-success-response` for the WebSocket operation in `.spectral.yaml` (`a374895`, 2026-11-17);
  - the exclusion of that operation from Schemathesis in `make/gates.mk` (`71aab46`, 2026-11-17);
  - the two `[advisories] ignore` entries of RUSTSEC-2026-0542 and RUSTSEC-2026-0543 in `deny.toml` (`c4f83b3`, 2026-11-20, section 9);
  - the plant of `jiff` in place of `humantime` in test G8 of `tests/CLI/bats/make_quality_gates.bats`: once story 1.4 removes `humantime` from `[workspace.dependencies]`, the old plant makes `cargo update --workspace --offline` fail, and `humantime` cannot stay there because `deny.toml` denies an unused workspace dependency (`49641e7`, 2026-11-20, section 9) [V `git show 49641e7`].
- The test oracles that change (section 2.5) are ordinary test files.

## 7. Risks

| Risk | Severity | Mitigation in the plan |
| --- | --- | --- |
| The HTTP status depends on the first poll, because execution is lazy (section 4) | high | the handler drives the operation until the resolver signals "opened" or "refused", or until the first result, under `HTTP_REQUEST_TIMEOUT`, and only then writes the head |
| Two root fields open two hub subscribers per request, with ambiguous ids | high | enforce the single-root-field rule before any resolver runs |
| `Accept` mis-negotiation breaks urql or streams its queries | medium | parse media ranges with their q values; tests for the graphql-sse, urql, EventSource, absent and `*/*` headers |
| `complete` without a `data:` line is ignored by `EventSource` | medium | byte-level body; a test on the exact bytes |
| The GraphQL stream reuses an event name that graphql-sse treats as fatal | medium | only `next` and `complete` are written; tests on the event names |
| A body re-sent by graphql-sse carries the first cursor | medium | the header wins; the docs show the `headers` function that sends `Last-Event-ID` |
| Per-subscriber resolution and serialisation of each event cost CPU | medium | it runs in the connection's task and never on the publish path; the hub still encodes one shared frame (bundle 4 NFR-2); the K6 scenario adds GraphQL subscribers |
| The id of a result is unknown when the client omits or aliases `id` | medium | the resolver hands each event's id to the transport through per-request data |
| Browsers allow 6 HTTP/1.1 connections per origin | low | documented; HTTP/2 at the edge recommended; single connection mode out of scope |
| A query inside an SSE body escapes the request budget | low | queries are driven to their result before the head |
| Template sync restores `handler.rs` | low | `.templatesyncignore` entry in the same story |
| Coverage and mutation at 100 % across a new transport | medium | unit tests on the wire module with paused time; mutation runs on the diff |
| Schemathesis refuses a streaming operation: a parameter schema that admits a value the service refuses, or refuses one it accepts; an event schema that does not match the frames; a 101 the client mishandles | medium | schemas written from the parsers (the cursor pattern, the type grammar, the handshake values); the event schema of one event; `make.schemathesis_validate` runs in the story; a failure is fixed at its root, else escalated with the exact exclusion (FR-29) |
| `Accept: */*` from Schemathesis gets the 405 of a GET of `/api/graphql` | high, found | the GET rule of FR-2: a wildcard admits the event stream on GET; a missing `Accept` keeps the 405 |
| The Schemathesis run grows by up to 10 s per silent streaming case | low | examples subscribe to `load.test`; the run ends by Schemathesis's own bounds (NFR-20) |
| A request-body string without `maxLength` gets a schema-valid case of 65 537 characters, which the size limit answers with the 413 (section 3.11) | high, found on 2026-11-21 | `query` at most 4 096 and `operationName` at most 128 characters, documented as `maxLength` and refused with the 422 problem on both transports and both forms (`56a8725`); the maintainer approved the change of the JSON transport (M5) |
| A route added later bypasses the document | medium | the source oracle fails on any `.route(`, `.route_service(` or `.nest_service(` call (FR-27) |
| A duration or an `occurredAt` value accepted today is refused after humantime leaves, or the reverse (sections 9.4, 9.5) | medium | every documented and tested duration keeps its `Duration`, its range and its reason, and every moved edge is pinned by a test (units of days and longer, `1M`, a repeated unit, a sign, a comma fraction, the clock form, a space before or inside a number, the bare `0`, which keeps its reading); the envelope keeps a fixed-position rule of its own and adds a test for every edge (leap second, day 31 of a 30-day month, February 29, more than nine fraction digits) |
| jiff 1.0.1 was published on 2026-11-18 | low | used only through `SpanParser::parse_unsigned_duration`, pinned by the duration tests; `make.check_security` checks its advisories and licence |
| The protected `architecture.toml` lets the application layer use `humantime` and not `jiff` | found | the envelope check needs no crate (section 9.5); jiff enters only the composition root, whose layer allows every crate |

## 8. Open questions resolved

> Assumption: only the distinct connections mode of graphql-sse is implemented; the single connection mode is not, because its reservation must stay on one node across the PUT, the stream, the POSTs and the DELETE, which needs sticky sessions or shared state that ADR 0014 excludes, and the protocol's own recipes say the distinct mode suffices for most uses.

> Assumption: the endpoint is the GraphQL endpoint `/api/graphql` itself, chosen by content negotiation and not by a sibling path, because the issue's criteria send the subscription and the query "to the GraphQL endpoint with `Accept: text/event-stream`", and graphql-sse clients and Yoga use one URL for both transports.

> Assumption: a request is answered as an event stream when its `Accept` header lists `text/event-stream` with a q value above 0 and above that of every `application/graphql-response+json` and `application/json` range it lists; every other POST (no header, `*/*`, `text/*`, a JSON type first or equal) keeps the JSON transport, whose answers are unchanged apart from the length bounds of decision M5 (amended on 2026-11-21). This streams the graphql-sse client (exactly `text/event-stream`), `EventSource` and urql's subscriptions, and keeps urql's queries, which list JSON types with the same q, on JSON. A GET, which has no JSON representation, follows the GET rule of the assumption on Schemathesis below (amended on 2026-11-17).

> Assumption: `GET /api/graphql` that negotiates SSE is served with the parameters `query`, `operationName`, `variables` and `extensions` (the last two JSON-encoded), as GraphQL over HTTP section 4.3 describes, so that a browser `EventSource` can subscribe and resume natively; a `GET` that does not negotiate SSE keeps today's status and body (the explorer in `dev`, otherwise the 405 problem with `Allow: POST`); its answers gain only `vary: accept`, and the 405 of other methods lists `GET, HEAD, POST`, so the existing router test and feature stay valid. JSON queries over GET are not added.

> Assumption: the subscription field is `events(types: [String!]!, lastEventId: String): Event!`, and `Event` has `id: String!` (the event id as a decimal string, because ids exceed both GraphQL `Int` and the integers a JavaScript number holds exactly), `type: String!` and `data: JSON!` (the event object, i.e. the envelope). This mirrors the WebSocket frame `{"id","type","data"}`. No description holds a word that the schema test refuses.

> Assumption: the `Last-Event-ID` header wins over the `lastEventId` argument, as on the REST streaming routes (bundle 4 FR-12): an `EventSource` reconnects with its original URL plus a newer header, and the graphql-sse client re-sends its first body, to which an application adds the header through the client's `headers` function.

> Assumption: invalid arguments are answered with `200 text/event-stream`, one `next` event whose data is `{"data":null,"errors":[...]}` with `extensions.code` `VALIDATION_FAILED`, then `complete`, then the end, and no hub subscriber is registered. The `types` rules and codes are those of the REST routes (`required`, `length`, `pattern`, `max_items`, property `types`). A malformed cursor gives one violation whose property is the carrier read (`Last-Event-ID` or `lastEventId`), whose message is the REST text "<carrier> must be a decimal unsigned 64-bit integer", and whose code is `format`. The protocol requires this: validation found before execution MUST travel through an accepted SSE connection. "No stream of events is opened" therefore means that no hub subscription and no event exist. The resolver builds a 422 problem and converts it with `problem_to_error`, so the shared helper stays unchanged.

> Assumption: a node that is starting, at its subscriber capacity or shutting down answers the subscription before any SSE head with the same 503 problem as the REST streaming routes (`detail` the hub's text, the `subscription rejected` log). Availability is not a validation step of the operation, and GraphQL over HTTP section 5.4 names 503 for load shedding. The graphql-sse client retries any non-2xx answer with backoff, possibly on another node, and the existing documentation already says a 503 is safe to retry on any node. No hub slot is taken.

> Assumption: the handler drives the operation before it writes the head until one of three things happens: the resolver reports that the subscription opened, the resolver reports a refusal, or the first result arrives. This runs under `HTTP_REQUEST_TIMEOUT`. A query is therefore executed within the request budget, a refusal can still become a 503, and no answer waits for a broker read, because opening a subscription reads no broker.

> Assumption: a subscription operation must select exactly one root field, and that field must not be an introspection field, as the GraphQL specification rule 5.2.3.1 requires; the service enforces it as the direct-field rule (exactly one field selection written directly at the root, no root fragment), because async-graphql makes one stream per direct root field and skips root fragments. The rule is checked before any resolver runs. A violation is answered like an invalid argument: one `next` with `VALIDATION_FAILED`, then `complete`, and no hub subscriber. async-graphql does not enforce the rule.

> Assumption: the GraphQL stream is written as bytes, not through `axum::response::sse`, because axum cannot write the bare `data:` line of `complete`. Its events are:
>
> - a subscription result: `event: next`, `id: <event id>`, `data: <result>`;
> - a query or error result: `event: next`, `data: <result>`;
> - the end: `event: complete` and a bare `data:`;
> - a keep-alive: the comment `:`.
>
> Each event ends with an empty line. The JSON is always compact, on one line.

> Assumption: the head is `200` with `content-type: text/event-stream`, `cache-control: no-cache` and `x-accel-buffering: no`, as on the existing SSE route. It has no `connection` header, which HTTP/2 forbids, and no `content-encoding: none`, which is not a registered content coding (the router has no compression layer).

> Assumption: the stream writes one comment right after the head, following the reference handler, so that browsers whose fetch waits for the first body bytes see the response at once. After that it writes a comment whenever `STREAM_KEEP_ALIVE` passes with nothing written, which meets the issue's keep-alive criterion.

> Assumption: a subscription stream that ends because the hub closed (shutdown, or the tail stopped) or because a replay failed ends without `complete`, so the client reconnects with its last id on any node; a failed replay writes `replay failed` with its own transport label. `complete` is written only after a single-result operation or an error result.

> Assumption: a query sent with `Accept: text/event-stream` answers one `next` with its result, then `complete`. A mutation cannot run, because the schema has no mutation root, so a mutation over GET cannot change state, and the reference's 405 for mutations over GET is not reproduced. A subscription sent without SSE negotiation keeps today's JSON answer ("Subscriptions are not supported on this transport.").

> Assumption: a request that is not a GraphQL request is refused before any stream, whatever it negotiates. On POST that is a body without a string `query`, answered with the 422 problem of today. On GET it is an unreadable or repeated parameter, answered 400 as on the SSE route, or `variables` or `extensions` that are not JSON objects, answered 422. Parse and validation errors of a GraphQL document go through the stream, as the protocol requires.

> Assumption: the transport learns the id of each subscription result from per-request data that the resolver fills before it yields the result, and not from the client's selection set, which may omit or alias `id`.

> Assumption: GraphQL resolution and serialisation of each event run in the subscriber's connection task. The hub keeps encoding one shared frame per event, and the publish path never waits for a GraphQL client (bundle 4 NFR-2, NFR-10). How the event object is taken from the frame (parsed per subscriber, or shared once parsed) is left to the architecture.

> Assumption: the code is placed like this:
>
> - the resolver goes in the event-stream application crate's `graphql` module, the only module there that may import `async_graphql` besides `http`, so it reaches `parse_names` and `Subscriptions::open` crate-internally;
> - the schema root and the GraphQL over SSE transport go in the composition root, which owns the schema and its transports (AD-18);
> - the handler that reads `Accept` and `Last-Event-ID` takes `HeaderMap`, the recorded streaming exception, and the composition root's extractor oracle is extended with it.

> Assumption: CORS needs no code change: the allow-list of `CORS_ALLOWED_ORIGINS` already allows `GET`, `POST`, `content-type` and `last-event-id`; without the variable the router has no CORS layer, so no other origin can read the stream. Credentials are not offered, because there is no subscriber authorization. Tests prove both cases for the POST and the `EventSource` form.

> Assumption: ADR 0021 "GraphQL subscriptions over Server-Sent Events" records the protocol choices and amends the "with no subscription" sentence of ADR 0007. The ADR index marks the amendment, as it marks 0005 superseded by 0020. Template ADR 0007 is not rewritten.

> Assumption: `.templatesyncignore` gains `crates/service/src/graphql/handler.rs`, plus `crates/service/src/graphql.rs` if that file is edited. `crates/shared/application/src/graphql.rs` stays unchanged and needs no entry.

> Assumption: the K6 subscriptions scenario gains held GraphQL SSE subscribers (POST with `Accept: text/event-stream`) within the existing `subscriptions` budgets, which are not protected. This shows that the new transport holds under the smoke profile that `make ci` runs.

> Assumption: the development explorer stays unchanged. Its fetcher has no SSE support, and running subscriptions from it is not a criterion. The documentation says subscriptions are run with a graphql-sse client, urql or `EventSource`.

> Assumption: the HTTP/1.1 limit of 6 connections per origin is documented together with the recommendation to serve HTTP/2 at the edge. It is accepted rather than addressed through the single connection mode.

> Assumption: every route the service serves, the documentation routes and `/ping` included, becomes an operation of the OpenAPI document (eleven operations), because the maintainer's decision of 2026-11-17 asks for every endpoint and for a test that fails on any route served without an entry.

> Assumption: every route is registered through `OpenApiRouter::routes(routes!(..))`, so the router and the document come from the same annotation, and a source oracle fails on any `.route(`, `.route_service(` or `.nest_service(` call; the composition root merges three routers (REST with `AppState`, GraphQL with `ApiSchema`, operational without state) into one router and one document.

> Assumption: the documentation asset route becomes `/api/docs/{file}`, one segment, because utoipa-axum writes the annotated path verbatim into the document and `{*file}` is not an OpenAPI path template; the embedded Swagger UI files are one segment deep [I], and the story proves that every asset the page loads is served.

> Assumption: a GET of `/api/graphql` whose `Accept` admits the event stream through a wildcard negotiates SSE, and a GET without `Accept` keeps today's answer; this fixes at its root the only Schemathesis conflict found (its `Accept: */*`), keeps every existing test that sends no `Accept`, and follows RFC 9110 12.5.1 for the wildcard and the "Accept" section of GraphQL over HTTP, which lets a server answer a request without `Accept` as it chooses (`prd.md` FR-2).

> Assumption: no exclusion from Schemathesis is planned. A failure on a streaming operation is fixed in the document or the code; one that cannot be fixed at its root stops the story and goes to the maintainer with the exact exclusion, which the plan never makes itself.

> Assumption: ADR 0022 "Every route in the OpenAPI document" records these decisions and amends ADR 0006 and ADR 0007; `API_VERSION` becomes 2.1.0.

> Assumption: humantime leaves the production build in one story, 1.4, placed at the end of Epic 1 with no dependency, because the maintainer asked for one new story in this pull request (M4) and the files it edits (the workspace and crate manifests, the lock, `config.rs` and its tests, `envelope.rs` and its tests, `docs/environment.md`) belong to no other remaining story.

> Assumption: `crates/service` parses its durations with `jiff::fmt::friendly::SpanParser::parse_unsigned_duration` of jiff 1.0.1, declared with `default-features = false` and the feature `std`, because jiff is the alternative the advisory recommends, its friendly format reads every documented and tested value (`500ms`, `2s`, `1m`, section 9.3) to the same `Duration`, it refuses negative values, and it needs no time-zone data. The units of days and longer, which humantime reads and jiff's duration parser refuses, are documented and used nowhere in the repository; they are refused with the unchanged reason and pinned by a test, and `docs/environment.md` names the accepted syntax. The bare `0` keeps its reading through a one-line guard, and the other moved edges of section 9.4 take jiff's answer, each pinned by a test.

> Assumption: the envelope's `occurredAt` check replaces `humantime::parse_rfc3339` with a fixed-position rule of its own and uses no crate: digits and separators at their byte positions, year 1970 to 9999, month 1 to 12, the day within its month with Gregorian leap years, hour at most 23, minute at most 59, second at most 60. The protected `architecture.toml` lets the application layer use `humantime` and not `jiff`, and the maintainer approved only the `deny.toml` change. jiff would also move the rule: its timestamps end at `9999-12-30T22:00:00.999999999Z` and its parsers read at most nine fraction digits, so it would refuse `9999-12-31T23:59:59Z`, which a test accepts today, and the longer fractions that humantime accepts.

> Assumption: the now unused `humantime` entry of the application layer's allow-list in `architecture.toml` stays. Removing it is a protected change that nobody approved, and the architecture gate does not report an unused entry (`xtask/src/arch/rules.rs:226`).

## 9. The RustSec advisories of 2026-11-20 (added on 2026-11-20)

Every fact of this section was verified on 2026-11-20 [V]: in the file and line named on branch head `c4f83b3`, in `Cargo.lock`, in the registry source of humantime 2.4.0, or in the crates.io index entries and the crate files of jiff 1.0.1 and jiff-core 1.0.1, fetched into the session scratchpad with the checksum of the index. No build or cargo command was run.

### 9.1 The advisories and the decision

- Two advisories published after 2026-11-17 made `make.check_security` (cargo deny, CI check "Dependency policy") fail on `main` and on this branch: RUSTSEC-2026-0542 (`humantime` 2.4.0 unmaintained, no safe upgrade) and RUSTSEC-2026-0543 (`chrono` 0.4.45 unmaintained, no safe upgrade; recommended alternatives `jiff` and `time`). Neither is a vulnerability. `deny.toml` sets `unmaintained = "all"` [V `deny.toml:14`].
- M4, the maintainer's decision of 2026-11-20:
  - (a) exactly two entries in `[advisories] ignore` of the protected `deny.toml`, committed by the orchestrator as `c4f83b3` "chore(#8): ignore the two unmaintained advisories in the dependency policy" [V `git show c4f83b3`]: RUSTSEC-2026-0542, "humantime is unmaintained, not vulnerable, and has no safe upgrade; it remains only through cucumber 0.23.0 (the newest release), a dev-dependency of the end-to-end tests. Remove when cucumber drops it."; RUSTSEC-2026-0543, "chrono is unmaintained, not vulnerable, and has no safe upgrade; it enters only through the test-util feature of aws-smithy-http-client, enabled by the dev-dependency aws-smithy-mocks, so the production binary never compiles it. Remove with SQS.";
  - (b) one new story in this pull request that replaces humantime in the two service crates with jiff, keeps the documented duration syntax and the envelope time rule, and leaves humantime only as cucumber's dev dependency, so that the reason of the first entry holds.

### 9.2 Where the two crates enter

- `chrono` 0.4.45 has one dependent in the lock, `cbor-diag` 0.1.12, reached through `aws-smithy-protocol-test` from `aws-smithy-http-client`, whose `test-util` feature the dev-dependency `aws-smithy-mocks` of the health-check infrastructure crate enables [V `Cargo.lock`, `crates/internal/health-check/infrastructure/Cargo.toml:17-19`]. The maintainer's check `cargo tree --workspace -e no-dev -i chrono` prints nothing. The plan changes nothing for chrono.
- `humantime` 2.4.0 has three dependents in the lock: `cucumber` 0.23.0, `event-stream-application` and `service` [V `Cargo.lock`]. `cucumber` 0.23.0 (2026-04-23) is its newest release and requires `humantime ^2.1` [V crates.io index]; only the `[dev-dependencies]` of `tests/e2e` name it [V `tests/e2e/Cargo.toml:12-15`].

### 9.3 What uses humantime today

- **Manifests.** `humantime = "2.4.0"` in `[workspace.dependencies]` [V `Cargo.toml:59`]; `humantime.workspace = true` in `crates/service/Cargo.toml:24` and in `crates/notification/event-stream/application/Cargo.toml:17` [V].
- **`crates/service/src/config.rs`.**
  - `ranged_duration` (`:725-734`) keeps a value that `humantime::parse_duration` reads and that lies in the rule's range; an unreadable value and one outside the range both get the reason `must be a duration from <range>`. Its six rules: `KAFKA_REQUEST_TIMEOUT` 1s to 30s, `KAFKA_CONNECT_TIMEOUT` 100ms to 30s, `KAFKA_FETCH_MAX_WAIT` 50ms to 5s, `KAFKA_RETRY_DELAY` 100ms to 1m, `EVENT_TAIL_STALE_AFTER` and `STREAM_KEEP_ALIVE` 1s to 5m [V `:686-721`].
  - `duration` (`:962-967`) has no range and the reason `must be a duration such as 500ms, 2s or 1m`. Its five variables: `SQS_CONNECT_TIMEOUT`, `SQS_OPERATION_TIMEOUT`, `HEALTH_PROBE_TIMEOUT`, `HTTP_REQUEST_TIMEOUT` and `SHUTDOWN_GRACE_PERIOD` [V `:367-420`].
- **The config tests.** `each_kafka_duration_accepts_its_bounds` computes its expected value with `humantime::parse_duration(raw)` [V `config/tests/kafka.rs:172`]. The values the tests, `.env`, `.env.test` and `docs/environment.md` name are `0s`, `49ms`, `50ms`, `99ms`, `100ms`, `300ms`, `500ms`, `999ms`, `1s`, `2s`, `3s`, `4s`, `5s`, `10s`, `15s`, `20s`, `30s`, `31s`, `61s`, `301s`, `3001ms`, `5001ms`, `1m`, `2m`, `5m` and `10m`, and the refused texts `""`, `-2s`, `fast`, `1 fortnight`, `later` and `soon` [V `config/tests.rs:92-123`, `:311-340`, `:525-557`, `config/tests/kafka.rs:154-202`, `config/tests/streams.rs:50-53`, `:108-111`, `:141-144`, `config/tests/env_file.rs:113`]. No file of the repository writes a duration with another unit [V `grep` over the compose files, `.env*`, the Makefile fragments, the workflows, the Bats tests and the features].
- **`crates/notification/event-stream/application/src/envelope.rs`.** `is_utc_time` (`:190-207`) strips a final `Z` or `+00:00`, requires the bytes after byte 19 of the rest to be nothing or a dot followed by one or more ASCII digits, then requires `humantime::parse_rfc3339(text).is_ok()` [V]. Its tests refuse 19 strings and accept 5 [V `envelope/tests.rs:243-281`].
- **Documentation.** `docs/environment.md:9` reads "Durations use the `humantime` syntax (`500ms`, `2s`, `1m`)." [V]. No other document, help text or source names humantime, apart from `architecture.toml:33` and `deny.toml` [V `grep`].

### 9.4 jiff 1.0.1

- **Release and licence.** The newest release is 1.0.1, published on 2026-11-18 (1.0.0 on 2026-11-15; 0.2.46 the last of 0.2), with `rust-version` 1.85, edition 2024 and the licence `Unlicense OR MIT` [V index, `Cargo.toml`]. `deny.toml` allows `MIT`, so cargo deny accepts it [V `deny.toml:21-32`]. Its one dependency that is neither optional nor tied to another platform is `jiff-core` 1.0.1 (renamed `jcore`), licence `Unlicense OR MIT`, whose own dependencies are all optional [V]. With `default-features = false` and the feature `std`, no time-zone database is compiled [V `[features]`].
- **What the lock gains.** `Cargo.lock` does not depend on the platform: it records every non-optional dependency of every target. jiff 1.0.1 has two such dependencies behind `cfg` [V `Cargo.toml`]: `jiff-static = "=1.0.1"` under `[target."cfg(any())".dependencies]` (never compiled), and `portable-atomic ^1.10.0` with `portable-atomic-util ^0.2.4` under `cfg(not(target_has_atomic = "ptr"))` (not compiled on x86_64 or aarch64). jiff-static 1.0.1 (`Unlicense OR MIT`) needs `jiff-core ^1.0.1`, `proc-macro2 ^1.0.93`, `quote ^1.0.38` and `syn ^3.0.3`; portable-atomic-util 0.2.8, the newest 0.2 release (`Apache-2.0 OR MIT`), needs `portable-atomic ^1.5.1` [V index, crates.io API]. The lock already holds `portable-atomic` 1.15.0, `proc-macro2` 1.0.107, `quote` 1.0.47 and `syn` 3.0.10, and no `jiff`, `jiff-static` or `portable-atomic-util` [V `Cargo.lock`]. The refreshed lock therefore gains exactly `jiff` 1.0.1, `jiff-core` 1.0.1, `jiff-static` 1.0.1 and `portable-atomic-util` 0.2.8 and raises no locked version; `deny.toml` allows `MIT` and `Apache-2.0` [V `deny.toml:21-32`].
- **The duration parser.** `jiff::fmt::friendly::SpanParser::new()` is a `const fn`, and `parse_unsigned_duration(input) -> Result<core::time::Duration, Error>` reads the friendly format [V `src/fmt/friendly/parser.rs:110`, `:289-307`]:
  - one or more numbers, each followed by a unit, from the largest unit to the smallest (`OutOfOrderUnits`), with optional spaces and commas, an optional decimal fraction and an optional sign;
  - a negative value fails, and so does any unit of days or longer (`NotAllowedCalendarUnit`) [V `src/fmt/util.rs:499-530`, `:1029`].
- **The units.** The labels are lower case and matched exactly: `h`, `hr`, `hrs`, `hour`, `hours`; `m`, `min`, `mins`, `minute`, `minutes`; `s`, `sec`, `secs`, `second`, `seconds`; `ms`, `msec`, `msecs`, `milli`, `millis`, `millisecond`, `milliseconds`; `us`, `µs`, `usec`, `usecs`, `µsec`, `µsecs`, `micro`, `micros`, `microsecond`, `microseconds`; `ns`, `nsec`, `nsecs`, `nano`, `nanos`, `nanosecond`, `nanoseconds`; and the calendar units `d`, `w`, `mo`, `y` with their long forms [V `src/fmt/friendly/parser_label.rs`]. `1m` is one minute, and `1M` is no unit.
- **Against humantime 2.4.0.** humantime's units below a day are all labels of jiff [V humantime `src/duration.rs:323-332`]. Every value of section 9.3 therefore reads to the same `Duration`, and every refused text stays refused [I from the two grammars]. Values that move:
  - accepted today and refused after the change: the units `d`, `day`, `days`, `w`, `wk`, `wks`, `week`, `weeks`, `M`, `month`, `months`, `y`, `yr`, `yrs`, `year`, `years`; units out of order (`30s1m`); a unit written twice (`1h1h`, 2 h today); a bare `0` (zero today through an explicit special case, `humantime-2.4.0/src/duration.rs:364-367`; jiff's `parse_unit_designator` returns `ExpectedUnitSuffix`); a leading space (` 2s`; humantime's `parse_first_char` skips Unicode spaces, jiff's `parse` reads a number first); a space inside a number (`1 0s`, 10 s today; humantime's digit loop skips spaces, `:132`); a space that is not ASCII between number and unit (`1\u{a0}s`; jiff's `is_whitespace` takes ASCII only, `src/fmt/friendly/parser.rs:640-642`);
  - refused today and accepted after the change: a comma fraction (`1,5s`, 1.5 s), a leading `+` (`+2s`, 2 s), and the clock form `H:MM:SS` (`01:30:00`, 5 400 s, `parse_hms_maybe`);
  - not moved: a dot fraction (`1.5s` is 1.5 s for both; humantime's `'.'` branch reads it, `:136-139`), spaces between a number and its unit and between units (`1 m`, `1m 30s`), and a negative value (refused by both).
- **The plan's answer to the moved edges.** The bare `0` keeps its reading through a one-line guard in `parse_duration`, because an unranged variable set to `0` would otherwise stop a node at startup after an upgrade; every other moved edge takes jiff's answer, because no file of the repository writes one (section 9.3). Each is pinned by a test of story 1.4.
- **Timestamps.** `jiff::Timestamp::MAX` is `9999-12-30T22:00:00.999999999Z` [V `src/timestamp.rs:350`, `:1404`]; the parsers read at most nine fraction digits [V `src/fmt/util.rs:776`]; a leap second is clamped [V `src/fmt/temporal/parser.rs:822`].

### 9.5 What humantime's RFC 3339 check accepts

- `parse_rfc3339` requires at least 20 bytes, `T` at byte 10 and a final `Z` or `+00:00`, then runs `parse_rfc3339_weak` [V `humantime-2.4.0/src/date.rs:84-93`], which requires [V `:108-197`, `:22-26`]:
  - `-` at bytes 4 and 7, `:` at bytes 13 and 16, and two ASCII digits for each of year (four), month, day, hour, minute and second;
  - year at least 1970, hour at most 23, minute at most 59, second at most 60, a 60 read as 59 at any time of day;
  - month 1 to 12, and day 1 to the length of the month, February having 29 days in a Gregorian leap year (divisible by 4 and not by 100, or by 400);
  - any number of fraction digits, the tenth and later read as 0;
  - at most the last second of 9999 on 64-bit targets.
- With the checks `is_utc_time` makes first, the accepted strings are exactly: the 19-byte body `YYYY-MM-DDTHH:MM:SS` under those field rules, then nothing or a dot and one or more digits, then `Z` or `+00:00` [I].
- Edges that no test pins today [I from the code]: a leap second is accepted (`2026-11-14T10:00:60Z`, `9999-12-31T23:59:60Z`); second 61 and minute 60 are refused; day 31 of a 30-day month, February 29 of 2026 and of 2100, month 00 and day 00 are refused; February 29 of 2024 and of 2000 is accepted; a fraction of more than nine digits is accepted; a non-digit or a sign inside a field is refused.

### 9.6 Gates, allow-lists and template sync

- `[bans.workspace-dependencies]` sets `unused = "deny"`, so humantime must leave `[workspace.dependencies]` once no member uses it; cucumber keeps its own requirement [V `deny.toml:50-54`].
- The application layer's `external` names `humantime` and not `jiff` [V `architecture.toml:28-42`], and `architecture.toml` is protected [V `protected-config.toml`]. The composition root allows every crate [V `architecture.toml:76-80`]. An unused entry is not reported [V `xtask/src/arch/rules.rs:226`].
- Every make target that compiles runs cargo with `--locked` [V `make/gates.mk:3-23`], and `make update` would raise other crates within their requirements [V `Makefile:184-185`]. No make target refreshes the lock for a manifest change alone, and nobody can write four checksummed `[[package]]` entries by hand. The rust-implementer agent may run `cargo tree -i <crate>` for dependency questions [V the plugin's `agents/rust-implementer.md:190`]; without `--locked` it resolves the changed manifests and writes only the minimal lock change, keeping every version already locked. Story 1.4 therefore runs `cargo tree -i jiff` once, right after its manifest edits and as its first step, then the maintainer's check `cargo tree --workspace -e no-dev -i humantime`; `## Constraints` of `architecture.md` names this one exception to its make-only rule.
- `make.check_security` runs `cargo deny --locked check` [V `make/gates.mk:11`, `Makefile:388-389`].
- Every file that story 1.4 edits is already listed in `.templatesyncignore` (`Cargo.toml`, `Cargo.lock`, `crates/service/Cargo.toml`, `crates/service/src/config.rs`, `crates/service/src/config/`, `crates/notification/`, `docs/environment.md`) [V].
