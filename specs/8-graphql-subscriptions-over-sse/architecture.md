---
stepsCompleted: [autonomous-create-architecture]
inputDocuments:
  - https://github.com/VilnaCRM-Org/notification-service/issues/8
  - specs/8-graphql-subscriptions-over-sse/research.md
  - specs/8-graphql-subscriptions-over-sse/product-brief.md
  - specs/8-graphql-subscriptions-over-sse/prd.md
  - specs/4-realtime-event-delivery-hub/architecture.md
date: 2026-11-16
author: bmad-autonomous-planning (create-architecture stage)
---

# Architecture: GraphQL subscriptions over Server-Sent Events (issue #8)

The issue gained two criteria on 2026-11-17 (every route in the OpenAPI document and on the Swagger page; the OpenAPI gates and Schemathesis green); the section "OpenAPI Document of Every Route" designs them (`prd.md` FR-26 to FR-29). The maintainer's decision M4 of 2026-11-20 on two RustSec advisories adds the section "Durations and Timestamps without humantime" (story 1.4, `prd.md` NFR-21) and the section "Protected Changes".

This document turns `prd.md` of this bundle into a design that can be coded story by story in the existing layers. The PRD binds the requirements, the protocol choices P1 to P20, the messages and the status codes. This document binds where each piece of code lives, how the GraphQL subscription field and the Server-Sent Events transport reuse the hub and the subscription state machine of bundle 4, the request flow, the error and 503 mapping, the contract changes, the tests per layer, the documentation and the decision record.

Every fact about the repository was read in the code on branch `claude/vilnacrm-notification-service-br7s47` at `c36f708`, those of the humantime replacement at `c4f83b3` (`research.md` section 9). Every fact about a library was read in the registry source of the locked version (`~/.cargo/registry/src/index.crates.io-*/<crate>-<version>/`), and each is cited in "Verified Library Facts" or where it is used. Decisions of this stage are marked `> Assumption:`. The PRD and this design agree on the direct-field rule of FR-14 and on the extractor exceptions of NFR-13.

Notation: `FR-<n>` and `NFR-<n>` are the requirements of this bundle's `prd.md`; `4/FR-<n>` and `4/NFR-<n>` are those of `specs/4-realtime-event-delivery-hub/prd.md`; `P<n>` are the protocol choices of the PRD's table "Protocol choices".

## Tech Stack

- Rust toolchain: channel 1.99.1 (`rust-toolchain.toml`), edition 2024, `rust-version` 1.99.1.
- Crates of `[workspace.dependencies]` that the change uses. One entry changes: story 1.4 replaces `humantime` 2.4.0 with `jiff` 1.0.1 (`prd.md` NFR-21); no other entry and no other workspace feature changes:

| Crate | Requirement (unchanged) | What the change uses it for |
| --- | --- | --- |
| `async-graphql` | 7.2.1, `default-features = false`, features `uuid` | `Subscription` and `MergedSubscription` derives, `SimpleObject` with a field complexity, `Json<T>` (the `JSON` scalar), `Value`, `Schema::execute_stream`, `Schema::data`, `Request::{data, parsed_query}`, `Response`, `ServerError`, `parser::types` (the parsed document of the one-root-field rule) |
| `axum` | 0.8.9, features `macros`, `ws` | `http::{HeaderMap, HeaderValue, header}`, `extract::{State, RawQuery}`, `body::Body::from_stream`, `middleware::{from_fn, Next}`, `response::{Response, IntoResponse}` |
| `futures-util` | 0.3.34, `default-features = false`, features `alloc` | `stream::{unfold, iter, once}`, `StreamExt::{next, chain, map}`, `BoxStream`; the event-stream application crate already depends on it; `crates/service` moves it from `[dev-dependencies]` to `[dependencies]` |
| `tokio` | 1.54.1, features unchanged; dev feature `test-util` | `sync::watch` (the phase of a request), `time::{sleep, Sleep, Instant}` (the keep-alive timer), `select!`; paused time in tests |
| `serde` | 1.0.229 | `Deserialize` of the WebSocket frame text into the event object (event-stream application) |
| `serde_json` | 1.0.154 | compact encoding of execution results; parsing the `variables` and `extensions` parameters of the GET form; test oracles |
| `thiserror` | 2.0.21 | the wiring errors of the transport and the resolver |
| `tracing` | 0.1.44 | the existing entries `subscription rejected` and `replay failed` |
| `utoipa` | 6.0.0, features `axum_extras`, `uuid`, `yaml` | `#[utoipa::path]` of every handler, `IntoParams` and `ToSchema` of the streaming parameters and event schemas, `Parameter::content`, `contentMediaType` and `contentSchema`, `OpenApi::merge` |
| `utoipa-axum` | 0.3.0 | `OpenApiRouter::routes`, `routes!`, `split_for_parts`, `into_openapi` |
| `jiff` | 1.0.1, `default-features = false`, features `std` (new, replaces `humantime` 2.4.0, which leaves `[workspace.dependencies]`) | `fmt::friendly::SpanParser::parse_unsigned_duration`, the duration variables of `crates/service/src/config.rs` (story 1.4) |
| `shared-application` | path | `Problem`, `Violation`, `ApiJson`, `graphql::problem_to_error` (all unchanged) |
| `event-stream-domain`, `event-stream-application` | path | `Sequence`, `Cursor`, `EventType`; the hub, `Subscriptions`, `Subscription`, `Frame`, `EventStreamState` |
| `http-body-util`, `rstest`, `tower`, `tokio-tungstenite`, `test-support` | as in the manifests | tests |

## Constraints

(a) Commit convention: commit each story with the hooks active as `<type>(#8): story N.M <title>` (type one of feat, fix, refactor, build, chore, ci, docs, style, perf, test). Stay on the branch the plugin checked out (`claude/vilnacrm-notification-service-br7s47`, pull request #9) and never commit on `main`; never push, the plugin command pushes; never bypass a hook.

(b) Story dispatch: a story whose `Depends on:` stories are done is independent.
When the rust-backend-sdlc:rust-implementer agent is available, independent stories may run as parallel subagents under the DB rule; otherwise implement stories sequentially under the same DB rule.
DB rule: parallel agents run only the DB-free targets `make.format` (make format), `make.lint_code` (make lint-code), `make.unit_tests` (make unit-tests), `make.architecture` (make architecture) and `make.complexity` (make complexity); one serialized verify step at a time runs the DB-backed targets `make.setup_test_db` (not available: capability absent), `make.integration_tests` (make integration-tests), `make.tests_with_coverage` (make tests-with-coverage), `make.e2e_tests` (make e2e-tests) and `make.sqlx_prepare` (not available: capability absent). A target that reads not available is skipped and reported.

- Scope: only issue #8; the four commits of issue #7 already on the branch stay as they are.
- Placement: code changes only in `crates/notification/event-stream/application/src/{envelope.rs,graphql.rs,graphql/,http.rs,http/dto.rs,http/handler.rs}` and `crates/service/src/{config.rs,graphql.rs,graphql/,http.rs,http/,openapi.rs,bootstrap.rs}`, plus tests, features, docs, `workspace.dsl`, `.templatesyncignore` and both snapshots; domain, infrastructure and shared crates stay unchanged.
- Protected: no story edits a protected file; the maintainer's approved changes are committed (`.spectral.yaml` a374895, `make/gates.mk` 71aab46, `deny.toml` c4f83b3, `tests/CLI/bats/make_quality_gates.bats` 49641e7). Manifests: `futures-util` moved (2.3); story 1.4 alone drops `humantime` everywhere and adds `jiff` 1.0.1 to `crates/service` only, `architecture.toml` unchanged. async-graphql keeps features `uuid` only, never `raw_value`.
- Advisories: no crate a RustSec advisory marks unmaintained in the build without dev edges; `make.check_security` passes (NFR-21).
- No suppression: no `#[allow]`, `#[mutants::skip]`, `#[coverage(off)]`, exclusion or lowered bar; `#[expect]` only with a reason.
- Tools only through make targets; never cargo, docker or a script directly, except story 1.4's lock refresh `cargo tree -i jiff`.
- Publish path untouched: `Frame`, `Hub::deliver`, the tail and the window do not change; GraphQL parsing, resolution and encoding run in the connection task.
- Before the head nothing reads the broker: a subscription takes no step until `StreamLink::release`; the phase before the head runs inside `HTTP_REQUEST_TIMEOUT`.
- Wire: only `next`, `complete` with a bare `data:`, the comment `:`, and `id:` on subscription results; bytes written by hand, never `axum::response::sse`; no `retry:`, no `connection` header.
- The JSON transport keeps its bytes; GET answers gain only `Vary: accept`; a GET whose `Accept` admits the stream by a wildcard gets the stream, one without `Accept` keeps today's answer; other methods get `Allow` from axum.
- Contracts: regenerate `.github/graphql-spec/spec` with `make.generate_graphql_spec` and `.github/openapi-spec/spec.yaml` with `make.generate_openapi_spec` in the story that changes each; no GraphQL description holds node, Connection, update, delete or "input ".
- OpenAPI: every route registered through `routes!` and documented; no Schemathesis exclusion beyond the approved one; a failure that cannot be fixed at its root is escalated to the maintainer.
- Tests: paused tokio time; every await bounded by `tokio::time::timeout`; wire blocks compared byte for byte; broker tests only in the serialized step.
- Quality: 100 % line and function coverage, zero surviving or timed-out mutants, clippy pedantic, cognitive complexity 8, 50 lines, 5 arguments.
- No new variable and no new log message; `replay failed` gets the transport `graphql-sse`.
- Template sync: `crates/service/src/graphql.rs` and `crates/service/src/graphql/handler.rs` join `.templatesyncignore` in the story that first edits them.

## Context and Goals

Today `/api/graphql` serves `POST` with the JSON transport (`crates/service/src/graphql/handler.rs:27-36`) and, in `dev` only, the explorer on `GET` (`crates/service/src/http/router.rs:35-38`). The schema is `Schema<Query, EmptyMutation, EmptySubscription>` (`crates/service/src/graphql/schema.rs:17`). A subscription sent today is answered `200` with the error `Subscriptions are not supported on this transport.` (async-graphql `src/schema.rs:501`).

Bundle 4 built everything a subscription needs except its GraphQL form:

- the hub with its cap, its readiness rule and its shutdown (`event-stream/application/src/http/hub.rs`);
- `Subscriptions::open`, which registers a subscriber synchronously and reads no broker (`http/subscription.rs:85-117`);
- the state machine `Subscription::next`, which resumes from the window, from Kafka or after the wait of a cursor ahead, then goes live, with strictly increasing ids on one connection (`http/subscription.rs:203-237`, `:313-497`);
- the shared `Frame`, encoded once per event (`http/frame.rs:10-49`);
- the type and cursor rules (`http/dto.rs:206-313`);
- the CORS allow-list (`crates/service/src/http/origin.rs:26-39`) and the send bound on every socket (`crates/service/src/command/serve.rs:110-126`).

This change adds one subscription field, one per-request link between the transport and the resolver, and one transport in the composition root, and it registers every route through the OpenAPI router so that the document lists all of them. Nothing else moves.

What the design must achieve, from the PRD:

- One transport choice by `Accept` on one path (FR-2), with a POST form (FR-3) and a GET form (FR-4), and every other request unchanged (FR-5, NFR-17).
- The graphql-sse wire format written byte for byte (FR-7, FR-8), with `id:` on subscription results (FR-10).
- The head written only after the operation registered, was refused or produced its first result, inside the request budget (FR-9), and never after a broker read (NFR-2).
- Validation answered inside the stream (FR-13 to FR-15), availability before it (FR-16).
- The guarantees, the limits and the slow-client handling of bundle 4 unchanged (FR-11, FR-12, FR-18, NFR-1, NFR-6).
- Every route the service serves an operation of the OpenAPI document and of the Swagger page, a test that fails for a route served without an entry, the OpenAPI gates green on every operation, and Schemathesis green on every operation but `GET /api/events/ws`, which only the approved `71aab46` excludes (FR-26 to FR-29, NFR-20).
- No crate that a RustSec advisory marks unmaintained in the production build, with the documented duration syntax and the envelope time rule unchanged (NFR-21).

No story edits a protected file (NFR-15). The four protected changes that implementation, the advisories of 2026-11-20 and the removal of `humantime` from `[workspace.dependencies]` showed to be needed were approved by the maintainer and committed outside the stories; "Protected Changes" lists them. No bounded context is added: both touched crates are live members of `[workspace].members`.

## Verified Library Facts

1. **async-graphql 7.2.1** (`async-graphql-7.2.1/src`):
   - `Schema::execute_stream` (`schema.rs:573-660`) is lazy. Nothing runs until the stream is first polled.
     - A parse, validation or limit error becomes one `Response::from_errors`, then the stream ends (`:585-603`).
     - An operation that is not a subscription yields exactly one result through the same `execute_once` as `Schema::execute`, then ends (`:606-627`).
     - A subscription collects one stream per root field and merges them with `select_all` (`:629-652`).
   - `collect_subscription_streams` (`subscription.rs:38-72`) makes a stream only for a `Selection::Field` written directly in the root selection set (`:44`). A fragment spread or an inline fragment at the root is skipped and runs nothing. Two field selections with the same response name make two streams.
   - The `Subscription` derive (`async-graphql-derive-7.2.1/src/subscription.rs:310-393`) runs the resolver only when its stream is first polled, inside `stream::once(..).try_flatten()` (`:385`). A resolver `Err` becomes one error item with `locations` and `path`, and the stream ends. Each item is resolved against the selection set with `StreamExt::then` (`:324`): strictly one item at a time, and the next item is pulled only when the outer stream is polled again.
   - `Request` (`request.rs`): serde `camelCase` with `operationName` (`:21-30`); `Request::data` inserts per-request data that a resolver reads with `Context::data` (`:90`); `Request::parsed_query` parses once and caches the document, which `prepare_request` then reuses (`:114`, `schema.rs:865-883`).
   - `Variables` and the request `Extensions` deserialize `null` as empty (`async-graphql-value-7.2.1/src/variables.rs:26-30`, `extensions.rs:13-19`); `Variables::from_json` exists (`variables.rs:51`).
   - `Schema::data::<D>()` reads data set with `SchemaBuilder::data` (`schema.rs:663-670`).
   - `Json<T>` is the scalar `JSON`, described "A scalar that can represent any JSON value.", and resolves with `to_value(&self.0)` once per selection of the field, so each alias of a `Json` field builds its own copy (`types/json.rs:79-104`). `Json<T>` derives `Eq` (`types/json.rs:19`), and `ConstValue` is `Eq` (`async-graphql-value-7.2.1/src/lib.rs:131`).
   - The complexity visitor counts every field occurrence, also those reached through a fragment spread (`validation/visitors/complexity.rs:50-76`, `validation/visitor.rs:779-794`). A field without its own complexity costs 1 plus its children; a `SimpleObject` field with `#[graphql(complexity = N)]` costs `N` (`async-graphql-derive-7.2.1/src/simple_object.rs:220-228`).
   - Validation answers `subscription { __typename }` with `Unknown field "__typename" on type "Subscription".` (`validation/visitor.rs:615-626`).
   - `ConstValue` (exported as `async_graphql::Value`) deserializes JSON objects into an `IndexMap`, so member order is kept whatever the features of `serde_json` (`async-graphql-value-7.2.1/src/value_serde.rs:43`, `:154-158`). Its serializer writes an object as a raw JSON value only under the crate feature `raw_value` (`value_serde.rs:29-36`), which is not a default feature and which no crate of the lock enables (`async-graphql-7.2.1/Cargo.toml` `[features]`).
   - `serde_json`'s `preserve_order` is enabled only through the optional dependency of `aws-smithy-http-client`, so it depends on feature unification. A `serde_json::Value` would sort members in a single-crate mutation run of the event-stream application crate. Read for this document in the manifests of every locked crate.
   - `Response` always writes `data` (`null` on a request error) and skips empty `errors` and `extensions` (`response.rs:8-30`). `Response::from_errors` (`:44`), `ServerError` with public `message`, `locations`, `path` and `extensions` (`error.rs:38-53`), and `Error::into_server_error(pos)` (`error.rs:308`) exist.
   - `async_graphql::parser` re-exports the parser (`lib.rs:215`) with `ExecutableDocument`, `DocumentOperations::{Single, Multiple}`, `SelectionSet { items }`, `Selection::{Field, FragmentSpread, InlineFragment}` and `Field::{name, alias, directives}` (`async-graphql-parser-7.2.1/src/types/executable.rs:11-27`, `:140-206`).
   - The derives `Subscription` and `MergedSubscription` are not feature-gated (`lib.rs:280`, `:292`). The SDL export writes `subscription: <name>` into the `schema` block (`registry/export_sdl.rs:227-236`).
   - async-graphql has no single-root-field validation rule (none among `validation/rules/`).
2. **axum 0.8.9 and axum-core 0.5.6:**
   - `Event::data("")` writes no `data:` line, because `write_buf` returns on an empty buffer (`axum-0.8.9/src/response/sse.rs:441-444`). axum sets `content-type: text/event-stream` and `cache-control: no-cache` only for its own `Sse` (`sse.rs:97-98`).
   - `Body::from_stream` takes any `TryStream` whose items convert into `Bytes` and whose error converts into `BoxError` (`axum-core-0.5.6/src/body.rs:59-68`). `String` and `&'static str` convert, and `Infallible` is an error.
   - `RawQuery(Option<String>)` is the query string as received, with the rejection `Infallible` (`src/extract/raw_query.rs:25-37`).
   - A method router with `get` also answers `HEAD` through it, with the body removed (`src/routing/method_routing.rs:1157-1158`). For other methods it writes the `Allow` header from every registered method: `GET` adds `GET` and `HEAD` (`:852-870`, `:1167-1173`).
3. **tokio 1.54.1:** `watch::Sender::send_replace` (`src/sync/watch.rs:1247`), `watch::Receiver::wait_for`, which checks the current value first (`:907`), `watch::Sender::subscribe` (`:1405`), `time::sleep` (`src/time/sleep.rs:123`) and `Sleep::reset` (`:336`).
4. **futures-util 0.3.34:** `stream::unfold` (`src/stream/unfold.rs:50`), `stream::once` (`src/stream/once.rs:20`), `stream::iter` (`src/stream/iter.rs:46`); `StreamExt::boxed` needs only `alloc`.
5. **tower-http 0.7.1:** the CORS layer appends its `Vary` values to the response and never overwrites a `Vary` set further in (`src/cors/mod.rs:795-801`).
6. **The repository** (this branch):
   - The request budget wraps only `next.run`, so it bounds the time to the response head (`crates/shared/application/src/http/middleware.rs:13-21`). When it expires, the handler future is dropped.
   - `problem_to_error` turns a 422 problem into `VALIDATION_FAILED` with `violations`, and every other status into `INTERNAL_SERVER_ERROR` (`crates/shared/application/src/graphql.rs:14-22`). The detail of `Problem::validation_failed` is `The request failed validation.` (`http/problem.rs:22`, `:93-103`).
   - `From<HubError> for Problem` gives the 503 with the hub's text and logs `subscription rejected` (`event-stream/application/src/http/hub.rs:48-60`).
   - `Subscription::failure` holds the error chain of a failed replay, and `log_replay_failed(transport, error)` writes `replay failed` (`http/subscription.rs:290-294`, `:520-524`).
   - The crate module rule allows only the `http` and `graphql` modules of the event-stream application crate to import `async_graphql` and `axum` (`lib.rs:8-10`).
   - The extractor oracle of the composition root allows `State<ApiSchema>`, `ApiPath<`, `ApiJson<`, `ValidatedQuery<` and `ValidatedJson<`, and checks every file under `src/http` and `src/graphql` that holds a `pub async fn` (`crates/service/src/http/router/tests.rs:401-419`).
   - The access log writes `url.path` only (`crates/service/src/http/trace.rs:33-38`).
   - `Sequence::new(0)` is refused, so event ids start at 1 (`event-stream/domain/src/value_object/sequence.rs:17`).
   - `EventStreamState.settings.keep_alive` is public (`event-stream/application/src/http.rs:32-50`), and `assemble_router` already puts the `EventStreamState` into the schema data (`crates/service/src/bootstrap.rs:260`).
   - `test_node` is `#[cfg(test)] mod test_node;`, private to the `http` module of the event-stream application crate (`http.rs:14-15`); its items are `pub(crate)`.
   - `crate::http::router::assemble(state, schema, config)` is `pub` (`router.rs:33`). `application()` builds its own `InMemoryQueryBus` from the catalog (`bootstrap.rs:221-222`), and `FakeQueryBus` answers at once (`tests/support/src/bus.rs:136-149`).
   - The router-test helper `has_cors_headers` counts any `vary` header as a CORS header (`http/router/tests.rs:690-695`); `EXPECTED_SDL` pins the full SDL (`graphql/schema/tests.rs:8`, `:60-62`).
   - cargo-mutants 27.1.0 (`make/tool-versions.mk:12`) mutates `>` into `==`, `<` and `>=` (`src/visit.rs:590`); `.cargo/mutants.toml` has `exclude_re = []`, and the plan adds no exclusion, so an equivalent mutant fails the gate.
   - The K6 helper's `baseUrl` already ends in `/api` (`tests/Load/utils/utils.js:15`).
   - The streaming routes are registered with a plain `.route(..)` (`event-stream/application/src/http.rs:65-66`), and `assemble` registers `/api/graphql`, `/ping` and the three documentation routes the same way (`crates/service/src/http/router.rs:59-63`); none of them is in the document. `openapi_json` serves `openapi::document()` (`http/docs.rs:32-34`). The load-test stack runs `APP_ENV: test` (`docker-compose.load_test.override.yml:5`).
7. **utoipa 6.0.0 and utoipa-axum 0.3.0:** `OpenApiRouter::routes` registers the annotated path verbatim in the axum router and in the document (`utoipa-axum-0.3.0/src/router.rs:215-249`); `OpenApiRouter::route` documents nothing (`:251`). `Parameter` has `content` (`utoipa-6.0.0/src/openapi/path.rs:840`) and a string schema has `content_media_type` and `content_schema` (`src/openapi/schema.rs:1352-1361`).
8. **Schemathesis 4.33.1** (the image of `make/tool-versions.mk:38`, read from its wheel): an event stream is read until it ends, 20 events arrive or 10 s pass, and is then checked as a response cut short (`schemathesis/core/transport.py:56-90`, `transport/requests.py:320-335`); each event `{data, event, id?, retry?}` is validated against the `text/event-stream` schema, with `contentMediaType` and `contentSchema` (`core/deserialization.py:223-256`, `specs/openapi/validation.py:140-163`); requests carry `Accept: */*` (`reporting/ndjson.py:70`); `positive_data_acceptance` expects 2xx, 3xx, 401, 403, 404, 409, 429 or 5xx, `negative_data_rejection` 400, 401, 403, 404, 405, 406, 409, 415, 422, 428, 429 or 5xx (`config/_checks.py:13-29`); a connection dropped before its response is resent once (`transport/requests.py:297-315`).
9. **jiff 1.0.1** (`research.md` section 9.4, read from the crate file of the index, licence `Unlicense OR MIT`, `rust-version` 1.85, edition 2024): `fmt::friendly::SpanParser::new()` is `const`, and `parse_unsigned_duration` returns a `core::time::Duration` (`src/fmt/friendly/parser.rs:110`, `:289-307`). It reads numbers with lower-case units from hours down to nanoseconds, largest first, with optional ASCII spaces, commas, a fraction (`.` or `,`), a sign and the clock form `H:MM:SS`; a negative value, a unit of days or longer, a repeated unit, a number without a unit and a leading space fail (`src/fmt/friendly/parser.rs:311-470`, `src/fmt/util.rs:73-99`, `:499-530`, `:1029`). `Timestamp::MAX` is `9999-12-30T22:00:00.999999999Z` (`src/timestamp.rs:350`), and fractions have at most nine digits (`src/fmt/util.rs:776`). Without default features and with `std`, its only other crate compiled on this platform is `jiff-core` 1.0.1 (`Unlicense OR MIT`). `Cargo.lock` records the non-optional dependencies of every target, so it also gains `jiff-static` 1.0.1 (`Unlicense OR MIT`, required as `=1.0.1` behind `cfg(any())`, never compiled) and `portable-atomic-util` 0.2.8 (`Apache-2.0 OR MIT`, behind `cfg(not(target_has_atomic = "ptr"))`, not compiled on x86_64 or aarch64); their own dependencies `jiff-core`, `proc-macro2`, `quote`, `syn` 3.0.10 and `portable-atomic` 1.15.0 are already locked at versions that satisfy them (`Cargo.toml` of jiff 1.0.1, `[target."cfg(any())".dependencies]` and `[target.'cfg(not(target_has_atomic = "ptr"))'.dependencies]`; the crates.io index).
10. **humantime 2.4.0:** `parse_duration` reads the units `ns` to `h` under the same labels as jiff, plus days, weeks, months (`M`) and years (`src/duration.rs:323-332`); it returns zero for the bare text `0` (`:364-367`), skips any Unicode space before and inside a number (`:132`, `:180`), reads a dot fraction (`:136-139`), refuses a sign, a comma and a colon (`:141-143`, `:181-183`), and adds a unit written twice (`:254-267`). `parse_rfc3339` checks the fixed positions of `YYYY-MM-DDTHH:MM:SS`, year at least 1970, month, day within the month (Gregorian leap years), hour 23, minute 59, second 60 read as 59 at any time, any number of fraction digits, at most the last second of 9999 (`src/date.rs:84-197`).

## Component Placement

Every new or changed component, in the order a request meets it:

| Crate and file | Component | New or changed | Requirements |
| --- | --- | --- | --- |
| `crates/service/src/http/vary.rs` (+ `vary/tests.rs`) | middleware `vary_accept` | new | FR-5 |
| `crates/service/src/http.rs` | `pub mod vary;` | changed | FR-5 |
| `crates/service/src/http/router.rs` | `GRAPHQL_PATH`; GET on `/api/graphql` in every `APP_ENV`; the `vary_accept` layer | changed | FR-4, FR-5 |
| `crates/service/src/graphql/handler.rs` | `execute` (POST), `subscribe` (GET outside `dev`), `subscribe_or_explore` (GET in `dev`), `explorer`, `to_request` | changed | FR-2 to FR-6 |
| `crates/service/src/graphql/negotiation.rs` (+ tests) | `negotiates_event_stream` (POST rule), `negotiates_event_stream_on_get` (GET rule) | new | FR-2, FR-29 |
| `crates/service/src/graphql/query_string.rs` (+ tests) | `read`, the GET form; `QueryStringLimit` and `limit` | new | FR-4 |
| `crates/service/src/graphql/single_root.rs` (+ tests) | `check`, the one-root-field rule | new | FR-14 |
| `crates/service/src/graphql/event_stream.rs` (+ tests) | `respond`, the keep-alive read from the schema data, the wire encoders and the body | new | FR-6 to FR-9, FR-16 to FR-20 |
| `crates/service/src/graphql.rs` | the four new module declarations and `pub(crate) use query_string::QueryStringLimit;` | changed | NFR-13 |
| `crates/service/src/graphql/schema.rs` (+ `schema/tests.rs`) | `Subscription` root, `ApiSchema`, `builder` | changed | FR-1 |
| `crates/service/src/bootstrap.rs` | `QueryStringLimit` in the schema data | changed | FR-4 |
| `crates/service/Cargo.toml` | `futures-util` in `[dependencies]` | changed | NFR-13 |
| `crates/notification/event-stream/application/src/graphql/subscription.rs` (+ tests) | `StreamPhase`, `StreamLink`, `EventObject`, `EventStreamSubscription` | new | FR-1, FR-9 to FR-13, FR-16, FR-19 |
| `crates/notification/event-stream/application/src/graphql.rs` | `mod subscription;` and its re-exports; `state(ctx)` visible to the submodule | changed | NFR-13 |
| `crates/notification/event-stream/application/src/http/dto.rs` | `CursorFault` and `read_cursor`, one cursor rule with two answers; `types_problem` visible in the crate | changed | FR-11, FR-13 |
| `crates/notification/event-stream/application/src/http.rs` | `#[cfg(test)] pub(crate) mod test_node;`, so the tests of `graphql/subscription.rs` use the test node; the streaming routes registered through `routes!` | changed | FR-23, FR-26, FR-27 |
| `crates/notification/event-stream/application/src/http/handler.rs`, `http/dto.rs` | `#[utoipa::path]` of `stream` and `ws`; `IntoParams` of `StreamParams` and `WsParams`; the header parameters and the event and frame schemas | changed | FR-26 |
| `crates/service/src/openapi.rs` (+ `openapi/tests.rs`) | `graphql_router`, `operational_router`, `document()` merging the three routers; `API_VERSION` 2.1.0; the tags `GraphQL` and `Documentation`; the completeness oracles | changed | FR-26 to FR-28 |
| `crates/service/src/graphql/handler.rs`, `http/ping.rs`, `http/docs.rs` | `#[utoipa::path]` of `execute`, `subscribe`, `subscribe_or_explore`, `ping`, `docs_page`, `openapi_json` and `docs_asset` (one segment) | changed | FR-26 |
| `crates/service/src/config.rs` (+ `config/tests.rs`, `config/tests/kafka.rs`) | `parse_duration`, used by `ranged_duration` and `duration` | changed | NFR-21 |
| `crates/notification/event-stream/application/src/envelope.rs` (+ `envelope/tests.rs`) | `is_utc_time` with its own calendar rule in place of `humantime::parse_rfc3339` | changed | NFR-21 |
| `Cargo.toml`, `Cargo.lock`, `crates/service/Cargo.toml`, `crates/notification/event-stream/application/Cargo.toml` | `humantime` out, `jiff` into `[workspace.dependencies]` and `crates/service` | changed | NFR-21 |

> Assumption: the resolver and its objects live in the event-stream application crate's `graphql` module, and the transport lives in the composition root. AD-18 puts the schema and its transports in the composition root. The crate's module rule lets only `http` and `graphql` import `async_graphql`, and the resolver reaches `parse_names`, `types_problem`, the cursor rule and `Subscriptions::open` crate-internally, as `EventStreamQuery` reaches `list_types`. The transport never names a type of the hub other than `HubError` and `Sequence`.

> Assumption: the new transport modules of the composition root are private modules with `pub(crate)` items. Only `handler.rs` holds `pub async fn` handlers, so the handler-source list of the router oracle keeps one GraphQL file. The one item that the sibling module `crate::bootstrap` names, `QueryStringLimit`, is re-exported by `pub(crate) use query_string::QueryStringLimit;` in `graphql.rs`, because a `pub(crate)` item of a private module is not reachable from a sibling module (E0603).

### Event-stream application crate

#### `http/dto.rs`: one cursor rule, two answers

The REST routes answer a malformed cursor with a 400 problem. The GraphQL field must answer it with `VALIDATION_FAILED` (FR-13, AC4), which `problem_to_error` gives only for a 422 problem. The carrier rule stays in one place:

```rust
/// A cursor carrier whose value is not a decimal unsigned 64-bit integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CursorFault {
    /// `Last-Event-ID` or `lastEventId`: the first carrier, the header then the parameter, whose
    /// value is not a cursor.
    carrier: &'static str,
}

impl CursorFault {
    /// "<carrier> must be a decimal unsigned 64-bit integer"; it never holds the value.
    fn message(self) -> String;
    /// The 400 problem of the REST routes, unchanged (4/FR-16).
    pub(crate) fn bad_request(self) -> Problem;
    /// The 422 problem with one violation: `propertyPath` the carrier, `code` `format`, the message.
    pub(crate) fn violation(self) -> Problem;
}

/// Every carrier that is present must hold a cursor (the header is checked first); when both do,
/// the header wins, else the one present, else none (4/FR-12).
pub(crate) fn read_cursor(
    header: Option<&HeaderValue>,
    parameter: Option<&str>,
) -> Result<Option<Cursor>, CursorFault>;

/// Unchanged signature and answers: `read_cursor(headers.get(CURSOR_HEADER), parameter)` with
/// `CursorFault::bad_request`.
pub(crate) fn parse_cursor(headers: &HeaderMap, parameter: Option<&str>) -> Result<Option<Cursor>, Problem>;
```

The private helper of today that reads one carrier is folded into `read_cursor`. `types_problem` (`dto.rs:280-286`) becomes `pub(crate)`. Every existing test of `dto.rs`, of the SSE route and of the WebSocket route passes unchanged.

#### `graphql/subscription.rs`: the link, the object and the field

**`StreamPhase` and `StreamLink`.** The transport must know three things that async-graphql does not report: whether the field registered its subscriber, whether the hub refused it, and the event id of each result. The link carries them, and it carries the `Last-Event-ID` header to the resolver.

```rust
/// Where the subscription of one request stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamPhase {
    /// No subscriber registered yet; also the phase of every query.
    Pending,
    /// The `events` field registered its hub subscriber.
    Opened,
    /// The hub refused the subscriber; nothing is registered.
    Refused(HubError),
    /// The transport wrote the head; the subscription may take its first step.
    Released,
}

/// The per-request link between a subscription transport and the `events` resolver. The
/// transport puts one clone into the request data and keeps the other.
#[derive(Debug, Clone)]
pub struct StreamLink { inner: Arc<LinkInner> }

#[derive(Debug)]
struct LinkInner {
    /// The `Last-Event-ID` header of the request, read by the resolver (FR-11).
    last_event_id: Option<HeaderValue>,
    /// Pending -> Opened -> Released, or Pending -> Refused.
    phase: watch::Sender<StreamPhase>,
    /// The event id behind the last result the field yielded; 0 when none.
    delivered: AtomicU64,
}

impl StreamLink {
    pub fn new(last_event_id: Option<HeaderValue>) -> Self;
    /// The current phase.
    pub fn phase(&self) -> StreamPhase;
    /// Completes once the phase is not `Pending` (`watch::Receiver::wait_for`).
    pub async fn decided(&self);
    /// `Opened` -> `Released`; called by the transport when the head is decided.
    pub fn release(&self);
    /// The id of the last yielded result, once: `Sequence::new(swap(0)).ok()`.
    pub fn take_delivered(&self) -> Option<Sequence>;
    pub(crate) fn last_event_id(&self) -> Option<&HeaderValue>;
    pub(crate) fn opened(&self);
    pub(crate) fn refused(&self, error: HubError);
    /// Completes once the phase is `Released`.
    pub(crate) async fn released(&self);
    pub(crate) fn delivered(&self, sequence: Sequence);
}
```

The atomic uses `Ordering::Relaxed`: the whole GraphQL stream is polled in the connection's task, so the writer and the reader are the same task.

> Assumption: the transport learns "opened" and "refused" through a `tokio::sync::watch` channel in per-request data, and the id of each result through an atomic in the same link. The resolver runs lazily inside the transport's poll of the response stream (Verified Library Facts 1), so a signal set by the resolver is the only reliable way to decide the status before the head. "Pending at the first poll" is not reliable, because `Subscription::next` yields cooperatively. The id must not come from the client's selection set, which may omit or alias `id` (FR-10).

> Assumption: a subscription takes its first step only after the transport called `release`, that is, after it decided to write the head. Without this gate, the first poll would run `Subscription::next` right after `open`, through `try_flatten`, and could start a Kafka page read or take a window page before the head. With it, the phase before the head registers a subscriber and reads nothing (FR-9, NFR-2), a subscription never produces a result before its head, and the three points of FR-9 are deterministic in tests.

**`EventObject`.**

```rust
/// An event of a subscribed type: the same object as a WebSocket frame.
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
#[graphql(name = "Event")]
pub struct EventObject {
    /// Event id: the Kafka offset of the event plus one, in decimal; the cursor of a resume.
    pub id: String,
    /// Event type name.
    #[graphql(name = "type")]
    pub event_type: String,
    /// The event object the producer wrote: envelope id, type, occurredAt and data.
    // More than half of the complexity limit 1 000: a document selects `data` at most once (FR-1).
    #[graphql(complexity = 501)]
    pub data: Json<async_graphql::Value>,
}

impl From<&Frame> for EventObject { /* id from frame.sequence, type from frame.event_type,
    data: the member `data` of frame.json parsed as async_graphql::Value */ }
```

`data` is taken by deserializing the shared WebSocket text `frame.json` (`{"id":..,"type":..,"data":<event object>}`) into a private struct `{ data: async_graphql::Value }`. serde skips the other two members. A parse failure cannot happen for a frame the hub encoded; it maps to `Value::Null` in the same expression and is documented as such.

> Assumption: the event object is parsed from the frame in the subscriber's task, and `Frame` gains no field. The publish path then does exactly what it does today (NFR-1). A frame from a replay is encoded per subscriber anyway (`subscription.rs:307-309`). Parsing into `async_graphql::Value` keeps the producer's member order whatever the features of `serde_json` are (Verified Library Facts 1), so `Event.data` equals the SSE `data` line member for member (FR-1).

> Assumption: `EventObject` derives `Eq` as well, because every field is `Eq` (`String`, and `Json<ConstValue>`) and `clippy::derive_partial_eq_without_eq` (nursery, enabled at warn and denied by `make.lint_code`) fires on a public type that derives only `PartialEq`; `EventTypeObject` and `EventStreamStatusObject` derive both for the same reason.

> Assumption: `data` carries `#[graphql(complexity = 501)]`, more than half of `MAX_COMPLEXITY` (1 000), so a document selects the event object at most once under any alias or fragment (FR-1, NFR-3, NFR-8), while `events(..) { id type data }` costs 504. The reason is a plain `//` comment, because a `///` line would join the GraphQL description of FR-1. The literal lives in the application crate, which cannot name the composition root's `MAX_COMPLEXITY`; the tests of both crates pin the relation (a second `data` refused, `{ id type data }` accepted).

> Assumption: `data` uses `Json<async_graphql::Value>` and not `serde_json::Value`. Both print as the scalar `JSON` with the description FR-1 names, but only the first keeps member order in a single-crate mutation run. Every field of `Event` is computed eagerly from the frame: the cost is one parse per event and subscriber, in the connection task, and the object needs no async resolver. Resolving `data` copies the value once more, and only once, because the document selects it at most once.

**`EventStreamSubscription`.**

```rust
/// Subscription root of the event stream.
#[derive(Debug, Default)]
pub struct EventStreamSubscription;

#[Subscription]
impl EventStreamSubscription {
    /// Streams every event of the listed types (1 to 100 names); with lastEventId or the
    /// Last-Event-ID header, every retained event after that id comes first.
    async fn events(
        &self,
        ctx: &Context<'_>,
        types: Vec<String>,
        last_event_id: Option<String>,
    ) -> async_graphql::Result<impl Stream<Item = EventObject>>;
}
```

The resolver, in this order:

1. `state(ctx)?` reads `EventStreamState`. `link(ctx)?` reads the `StreamLink` of the request. A missing one is a wiring fault: `problem_to_error(Problem::internal(&MissingLink))`, logged once by `Problem::internal`.
2. `parse_names(types.iter().map(String::as_str))`, mapped with `types_problem`. Then `read_cursor(link.last_event_id(), last_event_id.as_deref())`, mapped with `CursorFault::violation`. The first failure is returned as `problem_to_error(problem)`, which gives `VALIDATION_FAILED` with one violation (FR-13). The resolver registers nothing in this case.
3. `state.subscriptions.open(SubscriptionRequest { types, cursor })`:
   - On `Err(error)`, it calls `link.refused(error)` and returns `async_graphql::Error::new(error.to_string())` without logging. The transport turns the phase into the 503 problem and logs once (FR-16).
   - On `Ok(subscription)`, it calls `link.opened()` and returns the stream of step 4.
4. The stream is `stream::unfold` over the subscription and a clone of the link:
   - Its first step awaits `link.released()`.
   - Each step awaits `subscription.next()`. On `Some(frame)` it calls `link.delivered(frame.sequence)` and yields `EventObject::from(&*frame)`.
   - On `None` it calls `log_replay_failed(TRANSPORT, error)` when `subscription.failure()` holds an error, then ends. `TRANSPORT` is the constant `"graphql-sse"` (FR-19 (c), FR-22).

The resolver never logs `subscription rejected` itself, so a refusal is logged exactly once. The stream owns the `Subscription`, so dropping the response stream drops the `Subscriber` and frees the slot (`hub.rs:388-398`). That covers a client that closes, a budget that expires before the head and a `HEAD` request.

### Composition root

#### `graphql/schema.rs`

```rust
/// Subscription root.
#[derive(Debug, MergedSubscription, Default)]
pub struct Subscription(EventStreamSubscription);

/// The schema type of the service: no mutation root.
pub type ApiSchema = Schema<Query, EmptyMutation, Subscription>;

pub fn builder() -> SchemaBuilder<Query, EmptyMutation, Subscription>; // limits unchanged
```

`sdl()` is unchanged and now prints the SDL of FR-1. The module's existing `#![expect(clippy::useless_let_if_seq, ...)]` stays. A lint triggered by the `MergedSubscription` expansion gets an `#[expect]` with its reason, never an `#[allow]`.

#### `graphql/negotiation.rs`

```rust
/// True when `Accept` ranks `text/event-stream` above both JSON types (FR-2).
pub(crate) fn negotiates_event_stream(headers: &HeaderMap) -> bool;
```

- Every `Accept` field is read with `HeaderValue::to_str`. A field that is not visible ASCII counts as absent.
- The field values are split at `,`. Each range is split at `;` and trimmed.
- The type and subtype are compared case-insensitively against `text/event-stream`, `application/json` and `application/graphql-response+json`, and, for the GET rule below, `*/*`, `text/*` and `text/html`. For the POST rule, wildcard ranges match none of the three types.
- A `q` parameter (the name compared case-insensitively) is parsed as an RFC 9110 qvalue into thousandths, 0 to 1000. A range with an invalid qvalue is skipped, and other parameters are ignored.
- `s` is the highest qvalue of an event-stream range and `j` the highest of a JSON range; an absent type counts 0. The function answers `s > j`. Because `j` is never negative, `s > 0` follows, and a separate `s > 0` test would leave the equivalent mutant `s >= 0`, which no test can kill (Verified Library Facts 6).
- Integer thousandths avoid float comparison.

```rust
/// True when a GET's `Accept` admits the event stream (FR-2, GET rule): a field is present and
/// `max(s, w) > max(j, h)`, where `w` is the highest qvalue of `*/*` or `text/*` and `h` that of
/// `text/html`.
pub(crate) fn negotiates_event_stream_on_get(headers: &HeaderMap) -> bool;
```

- It shares the range parser of `negotiates_event_stream`, which reports the four maxima `s`, `j`, `w` and `h` in thousandths; the POST function keeps answering `s > j`.
- No `Accept` field answers `false`: GraphQL over HTTP (section "Accept") lets a server answer a request without `Accept` with an error or a media type of its choice, and the service keeps today's answer. A present field whose ranges are all invalid gives `0 > 0`, also `false`.
- The comparison is `max(s, w) > max(j, h)`; with every maximum at least 0, a separate `> 0` test would leave an equivalent mutant, as for the POST rule.

> Assumption: the GET rule differs from the POST rule because a GET of `/api/graphql` has no JSON representation: outside `dev` the event stream is its only representation, so a wildcard admits it (RFC 9110 12.5.1), while `text/html` keeps the explorer in `dev` and the 405 elsewhere. It removes at its root the 405 that Schemathesis's `Accept: */*` would get (Verified Library Facts 8), and every existing test, which sends no `Accept`, keeps its answer.

#### `graphql/query_string.rs`

```rust
/// The bound of a GET query string, `HTTP_MAX_BODY_BYTES`, set in the schema data by the bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueryStringLimit {
    /// The longest query string read, in bytes, as received (FR-4).
    pub(crate) max_bytes: usize,
}

/// The limit from the schema data; its absence is a wiring fault: the 500 problem, logged once.
pub(crate) fn limit(schema: &ApiSchema) -> Result<QueryStringLimit, Problem>;

/// The GET form (FR-4): the JSON object `{query, operationName, variables, extensions}` that the
/// POST form would carry, or the problem of the first rule that fails.
pub(crate) fn read(raw: Option<&str>, max_bytes: usize) -> Result<Map<String, Value>, Problem>;
```

The rules run in the order of FR-4:

1. **414.** `raw.len() > max_bytes` gives the 414 problem `The query string is too long.`.
2. **400.** The query string is split at `&` (empty pieces skipped), and each piece at its first `=`. Each name and value is decoded strictly: `+` is a space, `%` must be followed by two hex digits, and the bytes must be UTF-8.
   - A decoding failure gives `Failed to deserialize query string: invalid percent-encoding or UTF-8`, with `` in `<name>` `` added when the name decoded to one of the four known parameters.
   - A known parameter given twice gives `` Failed to deserialize query string: duplicate field `<name>` ``.
   - A missing `query` gives `` Failed to deserialize query string: missing field `query` ``.
   - Unknown parameters are ignored and never echoed.
3. **422.** `variables`, then `extensions`, must parse as a JSON object (`serde_json::Map`). The first that does not gives `Problem::validation_failed` with one violation: `propertyPath` the parameter, `code` `format`, `message` `<name> must be a JSON-encoded object`.

The result goes through the same `to_request` as a POST body, so both forms build the `async_graphql::Request` the same way.

> Assumption: the GET parameters are decoded by a strict parser of the composition root and not by `axum::extract::Query`. `serde_urlencoded` decodes lossily: it keeps an invalid `%` sequence and replaces invalid UTF-8. FR-4 asks for a 400 in those cases. The 400 texts follow the wording of the existing query-string rejections.

> Assumption: on the GET form, `variables=null` and `extensions=null` are answered with the 422 violation, because FR-4 asks for a JSON-encoded object. The POST form keeps today's rule, where async-graphql reads `null` as empty (Verified Library Facts 1).

#### `graphql/single_root.rs`

```rust
/// The one-root-field rule (FR-14) for the operation that runs. `Ok` when the document does not
/// parse, names no runnable operation, or selects a query or mutation: async-graphql answers those.
pub(crate) fn check(request: &mut async_graphql::Request) -> Result<(), ServerError>;
```

1. `request.parsed_query()` parses and caches the document. On a parse error, `check` returns `Ok`, and `execute_stream` reports the error.
2. The operation is chosen as `prepare_request` chooses it (`schema.rs:902-927`): by `operationName` among several, or the single one. When none is found, `check` returns `Ok`.
3. For a subscription operation, the root selection set must hold exactly one selection, and it must satisfy all three conditions:
   - it is a `Selection::Field`;
   - its name does not start with `__`;
   - it has no directive named `skip` or `include`.
4. A breach returns `problem_to_error(Problem::validation_failed(vec![Violation { property_path: "query", code: "single_root_field", message: "a subscription operation must select exactly one root field" }])).into_server_error(pos)`. `pos` is the position of the selection that breaks the rule: the second selection, the fragment, the introspection field or the field with the directive.

> Assumption: the rule is the direct-field rule of FR-14: one field selection written directly at the root, with no fragment at the root, the same response name twice counting as two. async-graphql 7.2.1 makes one stream per direct field selection and skips root fragments (Verified Library Facts 1). Under the looser wording of GraphQL specification 5.2.3.1, `{ events(..) { id } events(..) { type } }` would register two hub subscribers, and `{ ...F }` would run nothing. The direct-field rule keeps FR-14's goal, one request and one subscriber: it refuses more requests than the specification and never runs more or fewer than one subscriber. Fragments inside the selection set of `events` work as FR-10 says.

> Assumption: the rule is checked only on the event-stream transport, before `execute_stream`, and not as a schema extension. On the JSON transport a subscription that passes document validation keeps today's single error `Subscriptions are not supported on this transport.` whatever its root shape, and one that fails validation, such as `subscription { __typename }`, keeps async-graphql's validation error (FR-5), so NFR-17 holds for every request that does not negotiate SSE. The rule runs on the parsed document before async-graphql's validation, so a document that breaks both the rule and a validation rule is answered with the rule's error. FR-6 puts both in the same step.

#### `graphql/event_stream.rs`

```rust
/// Answers one request that negotiated SSE and passed the transport checks: an event stream, the
/// 503 problem of a refused subscriber, or the 500 problem of a schema without `EventStreamState`.
pub(crate) async fn respond(
    schema: &ApiSchema,
    request: async_graphql::Request,
    last_event_id: Option<HeaderValue>,
) -> Response;
```

`respond` first reads the keep-alive interval from the schema data: `schema.data::<EventStreamState>()` and its `settings.keep_alive` (`STREAM_KEEP_ALIVE`). A schema without that state is a wiring fault: the 500 problem of `Problem::internal(&MissingStreamState)`, logged once, before anything runs. It then runs the phase before the head (see "Request Flow") and builds one of two bodies:

- **Single result** (a query, an error result, a breach of the root rule): `stream::iter([KEEP_ALIVE, next_block(result, None), COMPLETE])`. All three chunks are known before the head. When the stream ended without an item, the middle chunk is absent.
- **Subscription:** `stream::once(KEEP_ALIVE)` chained with `stream::unfold(SubscriptionBody { responses, link, keep_alive, timer }, next_chunk)`. Each `next_chunk` runs `tokio::select! { biased; item = responses.next() => .., () = &mut timer => .. }`:
  - an item gives `next_block(&item, link.take_delivered())`;
  - the end of `responses` ends the body, with no `complete`;
  - the timer gives `KEEP_ALIVE`.
  - Every chunk resets the timer to `Instant::now() + keep_alive` (tokio time).

The encoders:

```rust
const KEEP_ALIVE: &str = ":\n\n";
const COMPLETE: &str = "event: complete\ndata:\n\n";
/// `event: next\n` [+ `id: <id>\n`] + `data: <compact JSON of the result>\n\n`.
fn next_block(result: &async_graphql::Response, id: Option<Sequence>) -> String;
```

`next_block` encodes with `serde_json::to_string`. It cannot fail for a `Response`; a failure would fall back, in the same expression, to the fixed text `{"data":null,"errors":[{"message":"Internal server error"}]}`.

The head of every event stream is `200` with:

- `content-type: text/event-stream`;
- `cache-control: no-cache`;
- `x-accel-buffering: no`;
- `vary: accept`;
- the body `Body::from_stream(chunks.map(Ok::<_, Infallible>))`.

> Assumption: the stream is written as bytes through `Body::from_stream`, because axum's `Event::data("")` cannot write the bare `data:` line of `complete` (Verified Library Facts 2). The head copies the three headers of the SSE route plus `vary`. It has no `charset`, no `connection` and no `content-encoding` (FR-7).

> Assumption: the transport reads the keep-alive from the `EventStreamState` that the bootstrap already puts into the schema data (Verified Library Facts 6), and not from a second settings value. The route state stays `State<ApiSchema>`, which the extractor oracle and every router test already use. One source of the interval means no duplicated setting, no bootstrap change for the POST form and no struct literal that a later story must extend. A schema built without the state, which only a test can build, answers the 500 problem. The GET form's bound is the one transport value that the state does not hold; it travels as `QueryStringLimit` (see `graphql/query_string.rs`).

#### `graphql/handler.rs`

```rust
/// POST: the JSON transport, or the event stream when `Accept` negotiates it (FR-3, FR-5).
pub async fn execute(
    State(schema): State<ApiSchema>,
    headers: HeaderMap,
    ApiJson(body): ApiJson<Map<String, Value>>,
) -> Result<Response, Problem>;

/// GET outside `dev`: the event stream when `Accept` negotiates it, else the 405 problem with
/// `Allow: POST` (FR-4, FR-5).
pub async fn subscribe(State(schema): State<ApiSchema>, headers: HeaderMap, RawQuery(query): RawQuery) -> Response;

/// GET in `dev`: the event stream when `Accept` negotiates it, else the explorer page.
pub async fn subscribe_or_explore(State(schema): State<ApiSchema>, headers: HeaderMap, RawQuery(query): RawQuery) -> Response;

/// The development explorer page with its content security policy (unchanged).
pub async fn explorer() -> Response;

/// The request for a body with a string member `query` (unchanged rule, now shared by both forms).
pub(crate) fn to_request(body: Map<String, Value>) -> Option<async_graphql::Request>;
```

- `execute` without SSE answers `Json(schema.execute(request).await)` exactly as today.
- `execute` with SSE builds the request with `to_request` (else the 422 problem of today), then calls `respond` with the `last-event-id` header value.
- `subscribe` and `subscribe_or_explore` decide with `negotiates_event_stream_on_get` and share a crate-private helper that, with SSE, runs `query_string::limit(&schema)`, then `query_string::read(query.as_deref(), limit.max_bytes)`, then `to_request`, then `respond`.
- The 405 of `subscribe` is `Problem::new(StatusCode::METHOD_NOT_ALLOWED)` with `Allow: POST`, the body and header of today's method-router answer.

> Assumption: the three GraphQL handlers that can answer an event stream take `HeaderMap`, the recorded streaming exception, and the two GET handlers also take `RawQuery`. Both extractors are infallible. `RawQuery` is the only way to apply the 414 bound to the query string "as received" and to parse it strictly. `ValidatedQuery` would reject a malformed query string before the handler could keep today's 405 for a GET that does not negotiate SSE. The extractor oracle names these two exceptions for the handlers `execute`, `subscribe` and `subscribe_or_explore` only, as the event-stream crate's oracle names its streaming handlers, and as NFR-13 states.

> Assumption: the router keeps today's split by environment. In `dev` it registers `get(subscribe_or_explore)`, otherwise `get(subscribe)`. No environment flag is added to the route state.

#### `http/vary.rs` and `http/router.rs`

```rust
/// Sets `Vary: accept` on every answer to a GET or HEAD of `/api/graphql`, also on the 503 of
/// the request budget and the 500 of the panic catcher; other requests pass unchanged (FR-5).
pub(crate) async fn vary_accept(request: Request, next: Next) -> Response;
```

- It uses `HeaderMap::insert`. Inside the CORS layer the only `Vary` value can be the event-stream head's own `vary: accept`, so `insert` never duplicates it. The CORS layer then appends its `Vary: origin` values (Verified Library Facts 5).
- `router.rs` gains `pub const GRAPHQL_PATH: &str = "/api/graphql";`, used by the route and the middleware.
- The middleware order becomes: request id ×3, security headers, trace, CORS, **`vary_accept`**, WebSocket origin check, panic catcher, request budget, body limit.

> Assumption: `Vary: accept` is set by a middleware outside the budget and the panic catcher, not by the handlers, so that FR-5's "every answer to a GET" holds literally, including the 503 and 500 that those two layers write after the handler is gone. A route-level layer sits inside the budget and could not reach them. POST answers pass unchanged, so the JSON transport gains no header.

#### `bootstrap.rs` and `Cargo.toml`

- `assemble_router` (`bootstrap.rs:260`) builds `schema::builder().data(event_stream.clone()).data(QueryStringLimit { max_bytes: config.max_body_bytes }).finish()`. The keep-alive of the event stream comes from the `EventStreamState` that the first `.data(..)` already holds. The test schemas of `bootstrap.rs` that serve the GET form add the same line.
- `crates/service/Cargo.toml` moves `futures-util.workspace = true` from `[dev-dependencies]` to `[dependencies]`. `Cargo.lock` already lists `futures-util` for `service`. The composition root allows every external crate (`architecture.toml:76-80`).

## Reuse of the Hub and the Subscription State Machine

| Bundle 4 piece | How the GraphQL subscription uses it | Code changed |
| --- | --- | --- |
| `parse_names`, `types_problem` | the `types` argument, with the same codes and messages (FR-13) | visibility only |
| the carrier rule of `parse_cursor` | `read_cursor`: the header wins over the argument (FR-11) | refactored, same answers |
| `Subscriptions::open` | one registration per request: the cap, `Closed`, `NotReady`, `AtCapacity` in that order (FR-16) | none |
| `Subscription::next` | the window, the Kafka replay under the permits, the cap `EVENT_REPLAY_MAX_EVENTS`, the wait of a cursor ahead, live delivery, fall-behind and catch-up, strictly increasing ids (FR-11, FR-12, FR-18) | none |
| `Subscription::failure`, `log_replay_failed` | `replay failed` with the transport `graphql-sse` (FR-19 (c), FR-22) | none |
| `From<HubError> for Problem` | the 503 problem and the `subscription rejected` entry (FR-16) | none |
| `Frame` | `id` from `sequence`, `type` from `event_type`, `data` parsed from `json` | none |
| `Hub::close`, `drained` | the subscription stream ends after its queued frames, and the 2 s drain wait includes it (FR-19 (b)) | none |
| `Subscriber` drop | frees the slot when the response stream is dropped (FR-19 (a)) | none |
| `TCP_USER_TIMEOUT` on every socket | ends a connection that accepts no data (FR-18) | none |
| the CORS allow-list | both forms; no change (FR-21) | none |

A GraphQL subscriber is therefore a hub subscriber in every respect: it counts in `subscribers` and against `MAX_SUBSCRIBERS`, it falls behind and catches up on its own connection, and the publish path never waits for it.

## Request Flow

```text
client --POST|GET /api/graphql--> request id, security headers, trace, CORS, vary_accept (GET/HEAD),
                                  WebSocket origin, panic catcher, request budget, body limit
  handler: POST negotiates_event_stream(Accept)? / GET negotiates_event_stream_on_get(Accept)?
    no:  POST -> JSON transport (today)   GET dev -> explorer   GET other -> 405, Allow: POST
    yes: POST -> ApiJson (415/413/400/422) -> to_request (422)
         GET  -> query_string::limit (500 when unwired) -> query_string::read (414/400/422)
              -> to_request
         -> respond: keep-alive from EventStreamState in the schema data (500 when unwired)
         -> single_root::check -- breach --> head, ":", next {errors VALIDATION_FAILED}, complete
         -> link = StreamLink::new(Last-Event-ID); responses = schema.execute_stream(request + link)
         -> phase before the head (inside HTTP_REQUEST_TIMEOUT):
              select! { biased; link.decided(), responses.next() }
              then by link.phase():
                Refused(e) -> 503 problem, detail e; log "subscription rejected"; responses dropped
                Opened     -> link.release(); head, ":", then per event "next" with id, ":" after
                              every STREAM_KEEP_ALIVE without a block; never "complete"
                Pending    -> head, ":", next {first result}, complete   (query, document error,
                              invalid argument)
```

How the phase before the head unfolds:

1. The first poll of `responses` runs `prepare_request`: parse, depth, complexity and validation. A request error is yielded as the first item while the phase is `Pending`, so it becomes a single result (FR-15).
2. For a query, `execute_once` runs to the end inside this poll or the next ones, and its result is the first item (FR-20). The query runs inside the budget.
3. For a subscription, the derive polls the resolver:
   - Invalid arguments yield an error item while the phase is still `Pending` (FR-13).
   - A refusal sets `Refused`, then yields an error item that the transport drops.
   - A registration sets `Opened` and returns the gated stream, which answers `Pending` until `release`. `link.decided()` then completes. The phase was set inside the same poll, so the `watch` wakes the task at once.
4. The decision is read from `link.phase()` after the select, never from which branch won. A refusal may arrive through the item branch, and the phase still says `Refused`.
5. When the budget expires first, the middleware drops the handler future. That drops `responses`, the resolver stream, the `Subscription` and the `Subscriber`, so no slot stays taken (FR-9).

FR-6's order follows from where each check sits:

1. CORS preflight (layer);
2. transport errors (extractor, `to_request`, `query_string::read`);
3. the root rule and document errors (in the stream);
4. invalid arguments (resolver, in the stream);
5. availability (the 503 problem);
6. the stream.

## Wire Format

| Block | Bytes | Written by |
| --- | --- | --- |
| keep-alive comment | `:\n\n` | first chunk of every event stream; then the subscription body's timer |
| subscription result | `event: next\nid: <event id>\ndata: <result>\n\n` | `next_block(result, Some(id))`, id from `StreamLink::take_delivered` |
| single result | `event: next\ndata: <result>\n\n` | `next_block(result, None)` |
| complete | `event: complete\ndata:\n\n` | after a single result only |

- `<result>` is `serde_json::to_string(&async_graphql::Response)`: compact, one line, with every line break inside a string escaped. Example: `{"data":{"events":{"id":"1043","type":"user.created","data":{...}}}}`, or `{"data":null,"errors":[...]}`.
- `<event id>` is `Sequence` in decimal, so it has no leading zeros.
- No `retry:` field and no event name other than `next` and `complete` is ever written.

## Error and 503 Mapping

| Condition | Detected by | Answer | Log entry |
| --- | --- | --- | --- |
| CORS preflight | CORS layer | preflight answer (FR-21) | none |
| POST: media type, size, JSON syntax, JSON shape | `ApiJson` | 415, 413, 400 or 422 problem, as today | none |
| POST: no string `query` | `to_request` | 422 `The request body is not a GraphQL request.` | none |
| GET: query string above `HTTP_MAX_BODY_BYTES` | `query_string::read` | 414 problem | none |
| GET: undecodable, duplicate known parameter, no `query` | `query_string::read` | 400 problem | none |
| GET: `variables` or `extensions` not a JSON object | `query_string::read` | 422 problem, one violation, `code` `format` | none |
| subscription breaks the root rule | `single_root::check` | stream: one `next` with `VALIDATION_FAILED`, `single_root_field`; then `complete` | none |
| parse error, unknown field or argument, wrong type, variable not coercible, ambiguous or unknown operation, depth above 16, complexity above 1 000 (a second selection of `Event.data` included), mutation | async-graphql in `execute_stream` | stream: one `next` with async-graphql's errors, without a code; then `complete` | none |
| invalid `types` | resolver, `parse_names` | stream: one `next` with `VALIDATION_FAILED`, `propertyPath` `types`; then `complete` | none |
| malformed cursor | resolver, `read_cursor` | stream: one `next` with `VALIDATION_FAILED`, `propertyPath` `Last-Event-ID` or `lastEventId`, `code` `format`; then `complete` | none |
| hub `Closed`, `NotReady`, `AtCapacity` | resolver, then `StreamLink::refused`, then the transport | 503 problem with the hub's text as `detail`, in precedence Closed, NotReady, AtCapacity | `subscription rejected` (warn), once |
| budget expired before the head | `request_timeout` | 503 `The request exceeded its time budget.`; nothing registered | none |
| schema built without `EventStreamState`, request that negotiated SSE | `event_stream::respond` | 500 problem, before any resolver runs | error chain, once (`Problem::internal`) |
| schema built without `QueryStringLimit`, GET that negotiated SSE | `query_string::limit` | 500 problem | error chain, once (`Problem::internal`) |
| request without `StreamLink` or schema without `EventStreamState`, reached only by the resolver's own tests (the transport always adds the link and answers a missing state first) | resolver | stream: one `next` with `INTERNAL_SERVER_ERROR`; then `complete` | error chain, once |
| replay below the window cannot read Kafka | resolver stream | the body ends, without `complete` | `replay failed`, transport `graphql-sse` |
| node shuts down after the head | `Subscription::next` returns `None` | the body ends after the queued results, without `complete` | none |

On GET, every row's answer also carries `Vary: accept`. Every answer carries the CORS headers for an allowed origin. No log entry, no problem and no error built by this change's code (the resolver errors of FR-13, the single-root error of FR-14, the wiring errors, the 503 and the transport problems) holds a value of the request: the document, the variables, a type name, a cursor or event data (NFR-8). The async-graphql document errors of FR-15 may quote the client's own request (a source line of a parse error; a field, argument, variable, fragment or operation name), exactly as the JSON transport does today, and they go only to the requesting client.

## Lifecycle

- **Keep-alive (FR-17).** The first chunk is `:\n\n`. The subscription body's `Sleep` is reset after every chunk. While `responses` is pending, because the subscription waits for a permit, a Kafka page, the tail of a cursor ahead or a live event, the timer writes `:\n\n` every `STREAM_KEEP_ALIVE`. Single-result bodies need no timer: their three chunks are ready at once.
- **Slow and stalled clients (FR-18, NFR-6).** The hub's `try_send` never waits. A full queue makes the subscriber fall behind, and it catches up from the window or Kafka on its own connection. Resolution and encoding happen in the connection's task, when hyper polls the body. A client that accepts no data is ended by `TCP_USER_TIMEOUT`, and the dropped body frees the slot.
- **Ends (FR-19).** A client close drops the body at once. At shutdown, `Hub::close` ends the queue after its frames, `Subscription::next` returns `None`, and the body ends without `complete`. The node's drain wait of at most 2 s includes these streams. A failed replay ends the body the same way and logs `replay failed`.
- **Memory and state (NFR-3, NFR-5).** A GraphQL subscriber holds what an SSE subscriber holds, plus the response stream with at most one execution result being encoded, because the body yields one chunk at a time. That result holds at most one copy of the event object plus its id and type, because `Event.data` costs 501 of the complexity limit 1 000. The link lives in the request data and the response stream and goes with them. Nothing is kept between requests, so any node serves any request and a client resumes anywhere.
- **HEAD.** axum serves `HEAD` through the GET handler and drops the body. A `HEAD` that negotiates SSE registers and frees a subscriber within the request; the head it returns is the event-stream head.

## Browser Access

The CORS layer of `CORS_ALLOWED_ORIGINS` already allows `GET` and `POST` and the request headers `content-type` and `last-event-id`, with a maximum age of 600 s (`origin.rs:32-38`). Without the variable, the router has no CORS layer. No code changes. The router tests prove FR-21 for both forms:

- the preflight of the POST form with `content-type` and with `content-type, last-event-id`;
- `Access-Control-Allow-Origin` on the event-stream head, the 503 problem and the transport problems;
- no `Access-Control-Allow-Origin` for another origin, `*` for `Any`;
- no CORS header and a 405 for `OPTIONS` without the variable.

## OpenAPI Document of Every Route

This section designs FR-26 to FR-29 and NFR-20: every route the service serves becomes an operation of the OpenAPI document, the Swagger page shows it, a test fails for a route served without an entry, the OpenAPI gates pass on every operation, and Schemathesis passes on every operation but `GET /api/events/ws`, which only the approved `71aab46` excludes.

### One registration per route

Every route is registered through `OpenApiRouter::routes(routes!(..))`, which takes the path and the method from the handler's `#[utoipa::path]` and adds the route to the axum router and the operation to the document in one call (Verified Library Facts 7). Three routers carry the routes, because their handlers need three different states:

```rust
/// REST routes of the contexts, nested under `/api` (unchanged signature; the event-stream router
/// now also holds `stream` and `ws`).
pub fn api_router() -> OpenApiRouter<AppState>;

/// `/api/graphql`: `routes!(execute, subscribe)`, or `routes!(execute, subscribe_or_explore)` in
/// `dev`; both GET handlers carry the same annotation, so the document does not depend on the
/// environment.
pub fn graphql_router(app_env: AppEnv) -> OpenApiRouter<ApiSchema>;

/// `/ping` and the three documentation routes, which take no state.
pub fn operational_router() -> OpenApiRouter;

/// The document: the three routers' documents merged, the `prod` variant of `graphql_router`;
/// served by `/api/docs/openapi.json` and exported by `openapi-export`.
pub fn document() -> utoipa::openapi::OpenApi;
```

- `assemble` builds the axum router from the same three routers with `split_for_parts()`: `rest.with_state(state).merge(graphql.with_state(schema)).merge(operational)`, then the two fallbacks and the middleware, unchanged.
- The event-stream router registers `.routes(routes!(handler::stream))` and `.routes(routes!(handler::ws))` in place of its two `.route(..)` calls, and its comment and the handler module's comment drop "not operations of the `OpenAPI` document".
- The documentation asset route becomes `/api/docs/{file}`, one segment: `routes!` writes the annotated path verbatim, and `{*file}` is not an OpenAPI path template.
- `API_VERSION` becomes `2.1.0`; the tags `GraphQL` ("GraphQL queries and subscriptions over Server-Sent Events.") and `Documentation` ("The API documentation page and its document.") join `EventStream` and `HealthCheck`.

> Assumption: three routers instead of one, because the GraphQL handlers take `State<ApiSchema>` (the extractor oracle and every router test rely on it) while the REST handlers take `AppState` and the operational ones nothing; utoipa merges their documents, and the axum routers merge after their states are applied. `AppState` gains no schema field.

### What each operation documents

| Operation | Parameters and body | Responses | Examples |
| --- | --- | --- | --- |
| `GET /api/events/stream` (`api_events_stream_get`) | `types` (query, required, the comma-separated grammar of the names), `lastEventId` (query) and `Last-Event-ID` (header), both the cursor grammar | `200 text/event-stream`: schema `SseFrame` of one event (`event` the type name, `id` the decimal id, `data` with `contentMediaType: application/json` and the event object as `contentSchema`), headers `cache-control` and `x-accel-buffering`; `400`, `422`, `503` problems; `default` | `types=load.test`; one frame |
| `GET /api/events/ws` (`api_events_ws_get`) | `types`, `lastEventId` (query, optional); `Connection`, `Upgrade`, `Sec-WebSocket-Version`, `Sec-WebSocket-Key` (header, required, the values the upgrade accepts); `Last-Event-ID` (header) | `101 Switching Protocols` with the headers `Upgrade`, `Connection` and `Sec-WebSocket-Accept`, its description holding the server frames (`{"id","type","data"}` and the error frame) and the client commands (`subscribe`, `unsubscribe`); `400` (an invalid cursor or a handshake the upgrade refuses), `403` (an `Origin` outside the allow-list), `422`, `426`, `503` problems; `default` | a handshake for `load.test`; one frame of each direction |
| `POST /api/graphql` (`api_graphql_post`) | body `application/json` schema `GraphQLRequest` (`query` required, `operationName`, `variables`, `extensions`, unknown members accepted); header `Last-Event-ID` | `200 application/json` schema `GraphQLResponse` (`data` any JSON value or null, `errors`, `extensions`); `200 text/event-stream` schema `GraphQLEvent` (`event` `next` or `complete`, `id` on subscription results, `data` the execution result as JSON, or empty for `complete`); `400`, `413`, `415`, `422`, `503` problems; `default` | the query `{ eventTypes { name } }`; the subscription to `load.test`; one `next` and `complete` |
| `GET /api/graphql` (`api_graphql_get`) | `query` (required), `operationName`, `variables` and `extensions` (query; the last two strings with `contentMediaType: application/json` and an object `contentSchema`); header `Last-Event-ID` | `200 text/event-stream` (`GraphQLEvent`); `200 text/html` (the explorer, `dev` only, so stated); `400`, `405` (a GET that does not negotiate an event stream, outside `dev`), `414`, `422`, `503` problems; `default` | the URL-encoded subscription to `load.test` |
| `GET /ping` (`ping_get`) | none | `200 text/plain` `pong`; `default` | `pong` |
| `GET /api/docs` (`api_docs_get`) | none | `200 text/html` with the content security policy header; `default` | none needed beyond the media type |
| `GET /api/docs/openapi.json` (`api_docs_openapi_json_get`) | none | `200 application/json`, an object; `default` | none needed beyond the media type |
| `GET /api/docs/{file}` (`api_docs_file_get`) | `file` (path) | `200` with `text/css`, `application/javascript`, `text/html` or `image/png`; `400`, `404` problems; `default` | `swagger-ui.css` |

- Every parameter schema admits the values the service accepts and refuses those it refuses, so Schemathesis's positive cases are accepted and its negative cases refused: the cursor as a decimal of the accepted range, the type list as the grammar of the names, the handshake headers as the values axum's upgrade checks.
- Every string member of a request body has a `maxLength` that the service enforces with the 422 problem. Schemathesis 4.33.1 sends 65 537 characters to a body string without one as a schema-valid case, so the size limit alone would refuse a schema-valid body with the 413 (the run on `9bf9de6`, ST202 on `POST /api/graphql`; fixed by story 5.1). The GraphQL `query` holds at most 4 096 characters and `operationName` at most 128, on both forms (`MAX_QUERY_CHARS` and `MAX_OPERATION_NAME_CHARS` in `crates/service/src/graphql/handler.rs`), sized so that the default `HTTP_MAX_BODY_BYTES` holds both at their bound in their longest encoding, twelve bytes per character.
- Every status code the route can answer is listed with its media type and schema; the problems reference `Problem`. Every operation keeps a `default` problem response, which also meets the protected Spectral rules that ask for a 2xx or `default` and a 4xx or `default` response. Spectral's built-in rule `operation-success-response` asks every operation for a 2xx or 3xx answer, which the WebSocket route cannot give: it answers 101. The maintainer approved on 2026-11-17 an override in the protected `.spectral.yaml` (commit `a374895`) that turns that one rule off for `GET /api/events/ws` only; every other rule keeps applying to it.
- The event schemas `SseFrame` and `GraphQLEvent` describe one event, the convention of OpenAPI 3.2 that Schemathesis reads (Verified Library Facts 8); `GraphQLEvent.data` admits the empty data of `complete` next to the JSON of a result.
- The descriptions carry the frame formats of `docs/streaming-protocol.md` in short, and the Swagger page renders them with the examples.

> Assumption: the 403 problem of the origin check is documented on the WebSocket route only. The check answers any request that carries `Sec-WebSocket-Key` with an `Origin` outside the allow-list (`crates/service/src/http/origin.rs:41-59`), and only WebSocket handshakes carry that header; documenting it on every operation would describe a request no client of the other routes sends.

> Assumption: the event and frame schemas are new `ToSchema` types next to the handlers that answer them (`SseFrame` in the event-stream application crate's `http` module; `GraphQLRequest`, `GraphQLResponse` and `GraphQLEvent` in the composition root's `graphql` module), used only by the annotations; they are documentation types, not used to encode, so the wire bytes stay those of FR-8 and of bundle 4.

### Completeness tests (FR-27, FR-28)

In `crates/service/src/openapi/tests.rs`:

- `every_route_is_registered_through_routes`: reads the non-test `.rs` files of `crates/service/src` and of the application crates whose routers `api_router()` nests (`crates/internal/health-check/application/src`, `crates/notification/event-stream/application/src`), and fails on any `.route(`, `.route_service(` or `.nest_service(`, naming the file and line. It replaces `leaves_the_liveness_route_and_the_docs_routes_out`.
- `documents_exactly_the_eleven_operations`: the paths, methods, operation ids and tags of `document()` equal those of FR-26; it replaces `documents_exactly_the_three_operations`, and the schema oracle lists the new schemas.
- `every_operation_is_served`: for each operation, a request built from its path (sample path values) and method through `assemble` in `dev`, `test` and `prod` reaches its handler: the answer is not the 404 problem of the `not_found` fallback and not the 405 problem of `method_not_allowed`.
- `the_served_document_is_the_exported_one`: `GET /api/docs/openapi.json` returns `document()`, whose YAML is the snapshot.
- `the_page_assets_are_served`: every asset the page loads (`docs.html`) answers 200 at `/api/docs/{file}`.
- `features/api_docs.feature` gains a scenario that finds each of the eleven operations in `/api/docs/openapi.json`.

### Schemathesis on every operation (FR-29, NFR-20)

`make.schemathesis_validate` keeps its protected flags (`--checks all --phases=examples,coverage`), amended only by the approved `--exclude-operation-id api_events_ws_get` (`71aab46`, see "Protected Changes"). What each streaming operation meets, from Verified Library Facts 8 (the WebSocket row as designed before that exclusion):

| Operation | Schema-valid requests | Schema-invalid requests |
| --- | --- | --- |
| `GET /api/events/stream` | `200 text/event-stream`, read to 20 events or 10 s; each event validated against `SseFrame` | 400 or 422 problems |
| `GET /api/events/ws` | `101` with the documented headers; the reused connection is dropped and the next request resent | missing or wrong handshake headers: 400; invalid cursor: 400; invalid types: 422 |
| `POST /api/graphql` | `Accept: */*` keeps the JSON transport: `200 application/json` with the result or document errors | 400, 413, 415, 422 problems; `query` or `operationName` over its `maxLength`: 422 |
| `GET /api/graphql` | `Accept: */*` negotiates the event stream (GET rule): a single result and `complete`, or a subscription stream read to 20 events or 10 s | missing `query`: 400; `variables` or `extensions` not an object: 422; `query` or `operationName` over its `maxLength`: 422; too long: 414 |
| `PUT`, `DELETE`, `PATCH` of any path | 405 with `Allow` | not applicable |

The story that completes the document runs the gate. A failure is fixed at its root in the document or the code. None is expected to need an exclusion; should one prove impossible to fix, the story stops and escalates with the failing check, the reason and the exact exclusion (an `--exclude-operation-id` or a per-operation setting, and the file it would change, `make/gates.mk` being protected), and the operation stays on the Swagger page. The one exclusion made, `GET /api/events/ws`, went this way and was approved by the maintainer (see "Protected Changes").

### Protected tests of the OpenAPI gates

- G17 removes the first operation-level `description` line, now that of `GET /api/docs` (paths sort `/api/docs` first), and Spectral still fails.
- G19 removes the required `title` of `Problem`, which every problem response references, and oasdiff still reports `response-required-property-removed`.
- G21 plants an undocumented path; the run, now longer by the streaming cases, still ends and fails naming it.
- `scripts/check-docs.sh` keeps accepting `/api/...` and `/ping` citations through the first segments of the snapshot's paths.

## Durations and Timestamps without humantime

This section designs story 1.4 (`prd.md` NFR-21, maintainer decision M4 of 2026-11-20). `humantime` leaves the production build: jiff parses the durations of the composition root, and the envelope parser checks its time with a rule of its own. No behaviour that `docs/environment.md` or a test names changes, and nothing else moves. The facts are in `research.md` section 9 and in Verified Library Facts 9 and 10.

### `crates/service/src/config.rs`

```rust
/// jiff's friendly duration format: numbers with the units `h`, `m`, `s`, `ms`, `us` or `ns`
/// (or their long names), largest unit first, such as `500ms`, `2s`, `1m` or `1m30s`.
static DURATION_PARSER: SpanParser = SpanParser::new();

/// A duration of that syntax or the bare `0`, or `None` for any other text, a negative value or a
/// unit of days or longer.
fn parse_duration(value: &str) -> Option<Duration> {
    if value == "0" {
        return Some(Duration::ZERO);
    }
    DURATION_PARSER.parse_unsigned_duration(value).ok()
}
```

- `ranged_duration` keeps a value that `parse_duration` reads and that lies in the rule's range; `duration` maps `None` to its reason. The six `DurationRule` constants, the two reasons `must be a duration from <range>` and `must be a duration such as 500ms, 2s or 1m`, and every call site stay as they are.
- `config/tests/kafka.rs`: `each_kafka_duration_accepts_its_bounds` gets an expected `Duration` per case (`Duration::from_secs(1)`, `Duration::from_millis(100)`, ...) in place of `humantime::parse_duration(raw)`, so the code under test is not its own oracle.
- `config/tests.rs`: a table test that reads, through `SHUTDOWN_GRACE_PERIOD` (unranged and under no cross-field rule, so every readable value reaches the result; `HTTP_REQUEST_TIMEOUT` must exceed `HEALTH_PROBE_TIMEOUT`, which `valid()` sets to 4 s, and serves the refusals only), every value of `research.md` section 9.3 that no range test reads (`300ms`, `500ms`, `2s`, `3001ms`, `1m` as 60 s, `2m`, `10m`) and compares the `Duration`; and every edge that the change could move, each pinned with its expected result (`research.md` section 9.4):
  - read: `0` as `Duration::ZERO` (kept by the guard), `+2s` as 2 s, `1,5s` as 1 500 ms and `01:30:00` as 5 400 s; and, as edges that do not move, `1.5s` as 1 500 ms and `1m 30s` as 90 s;
  - refused with `must be a duration such as 500ms, 2s or 1m`: `1d`, `2w`, `1M`, `1y`, `30s1m`, `1h1h`, ` 2s` (a leading space), `1 0s` (a space inside a number) and `1\u{a0}s` (a no-break space before the unit);
  - the existing refusals (`""`, `-2s`, `fast`, `1 fortnight`, `later`, `soon`, every value one step outside a range) pass unchanged.

> Assumption: the parser is one `static` `SpanParser` behind one private function that both readers call, so the grammar lives in one place and the mutation run meets one call. `parse_unsigned_duration` is used rather than `SignedDuration::from_str`, because it reads only the friendly format (no ISO 8601 `PT2S`) and refuses a negative value itself.

> Assumption: the bare `0` keeps today's reading, `Duration::ZERO`, through the one-line guard in `parse_duration`. humantime documents and implements `0` as its one value without a unit, jiff refuses it for want of a unit, and an operator who set an unranged variable such as `SHUTDOWN_GRACE_PERIOD`, `SQS_CONNECT_TIMEOUT` or `SQS_OPERATION_TIMEOUT` to `0` would otherwise see the node refuse to start after an upgrade; a ranged variable still refuses `0` through its range, as today. The other moved edges are not documented, used by no file of the repository (`research.md` section 9.3) and take jiff's answer: a sign, a comma fraction and the clock form are read; a repeated unit, a unit of days or longer and a space before or inside a number are refused with the unchanged reason.

### `crates/notification/event-stream/application/src/envelope.rs`

`is_utc_time` keeps its suffix strip and its fraction check, and `humantime::parse_rfc3339(text).is_ok()` becomes a private rule over the first 19 bytes of the body:

```rust
/// `YYYY-MM-DDTHH:MM:SS` from 1970 to 9999: digits and separators at their positions, a day of
/// the calendar (Gregorian leap years), hour 0 to 23, minute 0 to 59, second 0 to 60 (a leap
/// second is accepted at any minute, as before).
fn is_calendar_time(body: &[u8]) -> bool;
/// The days of `month` in `year`; 0 for a month outside 1 to 12.
fn days_in_month(year: u16, month: u8) -> u8;
/// Divisible by 4 and not by 100, or divisible by 400.
fn is_leap_year(year: u16) -> bool;
```

- The separators are `-` at bytes 4 and 7, `T` at byte 10 and `:` at bytes 13 and 16; every other byte from 0 to 18 is an ASCII digit, and each field is read from its digits.
- The year is at least 1970 (four digits keep it at most 9999); the day is from 1 to `days_in_month(year, month)`, which is 0 for month 0 or a month above 12; the hour is at most 23, the minute at most 59, the second at most 60.
- The documentation comment of `is_utc_time` states this rule and no longer names a crate. Each function stays within the complexity bars (cognitive complexity 8, 50 lines, 5 arguments).
- `envelope/tests.rs`: every existing case passes unchanged. The new cases are answered as `humantime::parse_rfc3339` answers them today (`research.md` section 9.5):
  - accepted: `2026-11-14T10:00:60Z`, `9999-12-31T23:59:60Z`, `2024-02-29T10:00:00Z`, `2000-02-29T10:00:00Z`, `2026-11-30T10:00:00Z`, `2026-12-31T23:59:59+00:00`, and fractions of 10 and of 30 digits;
  - refused: `2026-11-14T10:00:61Z`, `2026-11-14T10:60:00Z`, `2026-11-31T10:00:00Z`, `2026-04-31T10:00:00Z`, `2026-02-29T10:00:00Z`, `2100-02-29T10:00:00Z`, `2026-00-14T10:00:00Z`, `2026-11-00T10:00:00Z`, `0000-01-01T00:00:00Z`, `2026-11-14T10:00:00+05:30`, a letter in each field, a wrong separator at each of the five positions, a sign inside a field (`2026-+1-14T10:00:00Z`) and a full-width digit;
  - both sides of every bound, so that no mutant survives: year 1969 and 1970; month 0, 1, 12 and 13; day 0 and 1; the last day and the next for months of 28, 29, 30 and 31 days; hour 23 and 24; minute 59 and 60; second 60 and 61; the leap-year rules on 2024, 2026, 2100 and 2000.

> Assumption: the rule reads bytes and uses no crate. The application layer's allow-list in the protected `architecture.toml` names `humantime` and not `jiff`, and the maintainer approved no change to that file; besides, jiff's timestamps end at `9999-12-30T22:00:00.999999999Z` and read at most nine fraction digits (Verified Library Facts 9), so jiff would refuse `9999-12-31T23:59:59Z` (test `last_second_of_9999`) and the longer fractions that are accepted today. The fixed positions and bounds are those humantime checks (Verified Library Facts 10), so the rule accepts and refuses exactly the same strings.

### Manifests, the lock and the documentation

- `Cargo.toml`, `[workspace.dependencies]`: `humantime = "2.4.0"` is removed, and `jiff = { version = "1.0.1", default-features = false, features = ["std"] }` takes its place in the sorted list, between `futures-util` and `krafka`.
- `crates/service/Cargo.toml`: `humantime.workspace = true` becomes `jiff.workspace = true` in `[dependencies]`.
- `crates/notification/event-stream/application/Cargo.toml`: `humantime.workspace = true` is removed, and nothing is added.
- `Cargo.lock` is regenerated by cargo, never by hand and never through `make update`, because every make target that compiles runs cargo with `--locked` and `make update` would raise other crates. As the story's first step, right after the three manifest edits, the implementer runs once the rust-implementer's allowed dependency probe `cargo tree -i jiff` without `--locked`: it resolves the changed manifests, writes only the minimal lock change, keeps every version already locked, and shows `service` as the only dependent of `jiff`. It then runs the maintainer's check `cargo tree --workspace -e no-dev -i humantime` and records its empty output. The lock gains exactly `jiff` 1.0.1, `jiff-core` 1.0.1, `jiff-static` 1.0.1 (pinned with `=` behind `cfg(any())`, never compiled) and `portable-atomic-util` 0.2.8 (behind `cfg(not(target_has_atomic = "ptr"))`, not compiled on x86_64 or aarch64), and raises no version that is already locked; `service` lists `jiff` in place of `humantime`; `event-stream-application` lists neither; `humantime` 2.4.0 stays with `cucumber` as its only dependent. Until the lock matches the manifests, the `--locked` targets of the other stories of the wave fail in the shared tree, so the refresh comes first.
- `docs/environment.md` line 9 names the accepted syntax without a crate: numbers with a unit `h`, `m` (minutes), `s`, `ms`, `us` or `ns`, long names such as `min` or `seconds` allowed, largest unit first, for example `500ms`, `2s`, `1m` or `1m30s`, and `0` alone for zero; units of days and longer refused.
- Not changed: `deny.toml` (the approved entries are committed, `c4f83b3`), `tests/CLI/bats/make_quality_gates.bats` (the approved plant of `jiff` in test G8 is committed, `49641e7`), `architecture.toml` (its unused `humantime` entry stays), `tests/e2e` (cucumber keeps `humantime` as a dev dependency).

After the story the graph without dev edges holds no `humantime`, so the maintainer's check `cargo tree --workspace -e no-dev -i humantime` prints nothing and the reason of the first `deny.toml` entry holds; `make.check_security` passes (advisories with the two approved entries; licences, `jiff`, `jiff-core` and `jiff-static` (`Unlicense OR MIT`) and `portable-atomic-util` (`Apache-2.0 OR MIT`), each admitted through `MIT`; bans, no unused workspace dependency; sources); `make.architecture` and `make.unused_dependencies` pass with `architecture.toml` unchanged.

## Contract and Snapshot Changes

- **GraphQL snapshot** (`.github/graphql-spec/spec`), regenerated with `make.generate_graphql_spec` in the schema story:
  - it gains `type Event` (fields `data: JSON!`, `id: String!`, `type: String!` with the descriptions of FR-1), `scalar JSON` and `type Subscription` with `events(lastEventId: String, types: [String!]!): Event!`;
  - the `schema` block gains `subscription: Subscription`;
  - the query root does not change.
- **Breaking-change gate.** `make.graphql_diff` must report only non-breaking changes, so no `approved-breaking-change` label is needed. The research recorded that graphql-inspector rates an added subscription root and new types as non-breaking. The schema story runs the gate and stops for the maintainer if it reports otherwise.
- **The protected Bats test G20** removes the first single-line field of the first `type` block, now `data: JSON!` of `Event`, and still expects the diff to fail. Its file does not change.
- **Schema oracles** (`crates/service/src/graphql/schema/tests.rs`):
  - the ban on `type Subscription` is lifted;
  - the bans on `node`, `Connection`, `update`, `delete` and `input ` stay;
  - the first-type oracle names `Event`;
  - `EXPECTED_SDL`, the full-SDL oracle of `the_sdl_is_the_documented_schema`, becomes the full SDL of FR-1.
- **OpenAPI snapshot** (`.github/openapi-spec/spec.yaml`), regenerated with `make.generate_openapi_spec` in the stories that change the document: eight operations, two tags and the new schemas are added, `info.version` becomes `2.1.0`, nothing is removed. `make.openapi_diff` reports only non-breaking changes, and `make.validate_openapi_spec` and `make.schemathesis_validate` pass (see "OpenAPI Document of Every Route").
- `make.validate_contract_snapshots` is clean after the regeneration.

## Architecture Model Impact

`workspace.dsl` changes descriptions only, so no component and no relationship is added and `make.validate_architecture_model` keeps passing:

- the person `apiClient`: "... subscribes to events over Server-Sent Events, WebSocket, or GraphQL subscriptions over Server-Sent Events";
- the container `service`: "... GraphQL at /api/graphql, with subscriptions over Server-Sent Events, ...";
- the component `compositionRoot`: "... GraphQL schema and its Server-Sent Events transport ...";
- the component of `event-stream-application`: it adds "the GraphQL subscription field".

## Test Strategy

The general rules of bundle 4 apply:

- paused tokio time wherever a timer is involved;
- every awaited call bounded by `tokio::time::timeout`;
- fakes that pend after their script, so a mutant fails an assertion and never hangs;
- wire chunks compared byte for byte, and result JSON compared with `serde_json` or byte strings built from test structs;
- one Kafka topic per broker test.

**Event-stream application crate (unit, `graphql/subscription/tests.rs`, `http/dto/tests.rs`).** A test schema `Schema<EventStreamQuery, EmptyMutation, EventStreamSubscription>` holds the `test_node` hub, window and `Subscriptions`, plus the `StaticTopic` replay; `http.rs` declares `#[cfg(test)] pub(crate) mod test_node;` so that `graphql/subscription/tests.rs` can import it. Tests run `execute_stream` with a `StreamLink` in the request data. They cover:

- every row of FR-13's table (message, code, `propertyPath`, no subscriber, phase `Pending`);
- types checked before the cursor;
- the header winning over a valid argument, and a malformed argument refused also beside a valid header;
- a valid subscription registering exactly one subscriber and setting `Opened`;
- nothing stepping before `release`: no window page and no replay call, recorded by the scripted replay;
- the hand-over at the gate: a subscription without a cursor that registered (`Opened`) and is not released, with events of its type already in the window before registration; events published before `release` are yielded after it, exactly those, in id order, and none retained before registration;
- after `release`: window resume, Kafka resume and live delivery in id order, with no duplicate;
- `take_delivered` giving each result's id even when `id` is aliased or not selected;
- `EventObject::from(&Frame)` equal to the frame's event object, member order and non-BMP characters included;
- the complexity of `data`: in a test schema with `limit_complexity(1_000)`, `{ id type data }` runs, while `data` under two aliases or repeated through a fragment fails the complexity limit;
- each refusal setting `Refused(e)` and logging nothing;
- dropping the stream freeing the slot;
- a failed replay ending the stream and logging `replay failed` with `graphql-sse` (`LogRecorder`);
- a missing link or state giving `INTERNAL_SERVER_ERROR`;
- isolation (NFR-6): a GraphQL subscriber that is never polled falls behind while another subscriber of the type receives every event and `Hub::deliver` never waits.

The `dto.rs` tests cover `CursorFault::violation` and `read_cursor`. The existing `parse_cursor` tests stay unchanged.

**Composition root (unit):**

- `negotiation/tests.rs`: every row of FR-2's table for POST and for GET; case; `q` edge values `0`, `0.001`, `1.000`, `1.0001`, `2`; a non-ASCII field ignored; several `Accept` fields joined; on GET, no field, `*/*`, `text/*`, a browser navigation header and `application/json`, and the ties `text/html` = wildcard and JSON = wildcard.
- `openapi/tests.rs`: the completeness tests of "OpenAPI Document of Every Route".
- `query_string/tests.rs`: 414 at `max_bytes` and one byte above; each 400 text (bad `%`, `%` at the end, invalid UTF-8, a duplicate known parameter, a duplicate unknown parameter ignored, missing `query`); `+` decoded as a space; each 422; the order 414, then 400, then 422; the produced map.
- `single_root/tests.rs`: one field passes; aliases pass; the cases two fields, the same response name twice, a root fragment spread, a root inline fragment, `__typename`, `@skip` and `@include` each give the error at the right position; a parse error, an unknown operation and a query pass through.
- `event_stream/tests.rs` builds a started node from public APIs (`Hub`, `Window`, `Subscriptions`, `StatusSource`, `EventStreamState`, test-support fakes):
  - the three points of FR-9 under paused time: registered then head; refused then 503 with nothing registered; first result then head and `complete`;
  - what the budget does: the `respond` future dropped after the resolver registered and before it returned leaves `subscribers` at 0;
  - the keep-alive timing: the first comment at once, then one per interval, reset by a result;
  - the four blocks byte for byte;
  - a result whose strings hold line breaks, quotes and non-BMP characters, still on one line;
  - the end without `complete` on `Hub::close`;
  - NFR-2: with a replay fake that never answers, a cursor below the window still gets its head and then keep-alives;
  - `Event.data` under two aliases or repeated through a fragment: one `next` with async-graphql's complexity error, then `complete`, `subscribers` unchanged; `{ id type data }` runs;
  - under a `LogRecorder`, no entry for a query, a document error, an invalid argument and a subscription that opens, delivers one event and ends by client close (FR-22, NFR-9);
  - a schema without `EventStreamState` giving the 500 problem.
- `handler/tests.rs`: the JSON transport unchanged for every `Accept` that does not negotiate SSE, including subscriptions on JSON of every root shape and `subscription { __typename }` (FR-5); POST and GET with SSE; a schema without `QueryStringLimit` giving the 500 problem on a GET with SSE; the explorer and the 405 with `Allow: POST`.
- `vary/tests.rs`: GET and HEAD of the path get `Vary: accept`; POST and other paths do not; no duplicate value on an event-stream head.

**Router tests (`http/router/tests.rs`):**

- GET with SSE served in `dev`, `test` and `prod`;
- GET without SSE unchanged with `Vary: accept`;
- `Allow` of `PUT` and `DELETE` listing `GET`, `HEAD` and `POST`;
- the budget 503 of a query over SSE: the router comes from `assemble(state, schema, &config)` with a hand-built `AppState` whose `EventStreamState` holds a local fake `QueryBus` of the router tests whose `ask_erased` pends for ever (`std::future::pending`), and a schema built with `builder().data(that state)`; the POST form, and the GET form with `Vary: accept`;
- the panic 500 of a GET carrying `Vary: accept`;
- a GraphQL subscription that outlives the budget: a started application on a `StaticTopic` the test keeps, with `HTTP_REQUEST_TIMEOUT` 1 s and `STREAM_KEEP_ALIVE` 2 s under paused time; 5 s after the head the body still writes `:\n\n` every interval, delivers an event appended then, and `subscribers` still counts it;
- the CORS cases of "Browser Access", with `has_cors_headers` narrowed to an `access-control-*` header or a `vary` value `origin`;
- the extractor oracle extended with `HeaderMap` and `RawQuery` for the three named handlers only;
- the middleware order.

**Integration tests against the broker** (`crates/service/tests/integration/graphql_sse.rs`, new; `main.rs` declares it; `client.rs` gains a raw HTTP/1.1 POST event-stream client that parses `next`, `complete` and comment blocks). They run under the serialized verify step:

- a served binary delivers an event of type `a` and not one of type `b` (AC2);
- resume with the argument and with the header, from the window and from Kafka with a small `EVENT_WINDOW_MAX_EVENTS` (AC3);
- resume on a second node of the same topic;
- a shutdown that ends the stream without `complete`, then a resume that misses no retained event (FR-19);
- `MAX_SUBSCRIBERS` filled by a mix of SSE, WebSocket and GraphQL subscribers, after which the next request of each transport is refused (FR-16). The WebSocket subscriber is a raw upgrade handshake that `client.rs` gains, built on the existing `limit::dial` (GET `/api/events/ws` with `upgrade`, `connection`, `sec-websocket-key` and `sec-websocket-version`); it keeps the 101 connection open as the subscriber and reads the 503 head when refused, so no new `#[expect]` is needed.

**End to end** (`features/graphql_subscription.feature`, new). `tests/e2e/world.rs` gains `open_graphql_sse_post` and `open_graphql_sse_get` and a block parser that accepts `event`, `id`, `data` and comments. The steps go in `tests/e2e/steps/graphql.rs`. There is one scenario per acceptance criterion in the POST form and the GET form:

- delivery of the subscribed type only;
- resume by argument and by header;
- the five invalid subscriptions with `VALIDATION_FAILED`, `complete` and an unchanged `subscribers`;
- the 503 of a starting node, using a topic that does not exist yet;
- the keep-alive comment;
- a query with one `next` and `complete`;
- an allowed origin receiving `Access-Control-Allow-Origin`.

`features/graphql.feature` and `features/http_errors.feature` pass unchanged.

**Coverage and mutation.** One coverage run over `--lib --bins --test integration` must report 100 % of lines and functions; the Gherkin suite is not instrumented, so unit and integration tests reach every line. The diff mutation run must report zero survivors. Mutants on the qvalue parser, the decoder, the root rule and the wire encoders are killed by the table tests above, and those on the duration reader and the envelope's calendar rule by the tests of "Durations and Timestamps without humantime".

## Load Test

`tests/Load/scripts/subscriptions.js` holds, in every profile, one GraphQL subscriber per iteration next to the SSE and WebSocket ones:

- `` http.asyncRequest('POST', `${baseUrl}/graphql`, body, { headers: { Accept: 'text/event-stream', 'Content-Type': 'application/json' }, timeout: holdSeconds + 's', tags: { name: 'POST /api/graphql (subscription)', test_type: 'held_subscription' } }) ``, with the body `{"query":"subscription { events(types: [\"load.test\"]) { id type data } }"}`;
- the URL follows the existing `` `${baseUrl}/events/stream` ``: the helper's `baseUrl` already ends in `/api` (Verified Library Facts 6), so `/api/graphql` is reached without a second `/api`;
- checked to have received an `event: next` whose data holds `load.test`; the SSE route's probe already showed that the pinned K6 returns the body received before its own timeout.

The `endpoints.subscriptions` budgets of `tests/Load/config.json.dist` stay unchanged, and `make.smoke_load_tests` passes. `docs/load-testing.md` describes the GraphQL subscriber.

## Documentation

| File | Change |
| --- | --- |
| `docs/streaming-protocol.md` | new section "GraphQL subscriptions over Server-Sent Events", with the contents of FR-25: field and object, negotiation and both forms, head and the four blocks, queries, the answers before a stream and the error results in it, cursor carriers, keep-alive and end, reconnect per client (graphql-sse distinct mode with a `headers` function for `Last-Event-ID`; `EventSource` listeners for `next` and `complete`, with `close()` on `complete`; urql `fetchSubscriptions`; Apollo through a link), the HTTP/1.1 limit of 6 connections and HTTP/2 at the edge; the contents list, the answers table and the limits table extended |
| `docs/requests.http` | a POST subscription, the same with `Last-Event-ID`, a URL-encoded GET subscription and a query over SSE |
| `docs/adr/0021-graphql-subscriptions-over-server-sent-events.md` | new (see "Decision Record") |
| `docs/adr/README.md` | a row for 0021; the row of 0007 marked "(amended by 0021)" |
| `docs/architecture.md` | the GraphQL transport, the phase before the head inside the request budget, the `vary_accept` layer in the middleware order |
| `docs/operations.md` | the transport value `graphql-sse` of `replay failed`; what the edge proxy needs on `/api/graphql`: no buffering of `text/event-stream`, an idle timeout above `STREAM_KEEP_ALIVE`, HTTP/2 recommended |
| `SECURITY.md` | the GraphQL limits among the bounded inputs, `Event.data` at most once per document included; no authorization for the GraphQL subscription either; the GET form needs no preflight and can change no state; async-graphql's document errors may quote the client's own request back to that client, as on the JSON transport, while no log entry, problem or error of the service holds a value of the request |
| `docs/load-testing.md` | the GraphQL subscriber |
| `workspace.dsl` | the descriptions of "Architecture Model Impact" |
| `.templatesyncignore` | `crates/service/src/graphql.rs`, `crates/service/src/graphql/handler.rs`, `crates/service/src/http/ping.rs`, `crates/service/src/http/ping/`, `crates/service/src/http/docs.rs` and `crates/service/src/http/docs/` |
| `docs/adr/0022-every-route-in-the-openapi-document.md` | new (see "Decision Record") |
| `docs/adr/README.md`, `docs/architecture.md`, `docs/streaming-protocol.md` | a row for 0022, the rows of 0006 and 0007 marked "(amended by 0022)"; "outside the OpenAPI document" replaced by a pointer to the Swagger page; the GET rule of the negotiation and its `dev` effect on curl |
| `docs/environment.md` | line 9: the accepted duration syntax without naming a crate (story 1.4, see "Durations and Timestamps without humantime") |

`make.docs_check` passes in the story that changes each document.

## Decision Record

**ADR 0021, "GraphQL subscriptions over Server-Sent Events"** (`docs/adr/0021-graphql-subscriptions-over-server-sent-events.md`), in the form of the existing records (context, decision, consequences, planning decisions). It records:

1. **Protocol:** graphql-sse in its distinct connections mode on `/api/graphql`, chosen by `Accept` parsed as media ranges with q values. There is no single connection mode, because its reservation needs sticky sessions or shared state, which ADR 0013 and ADR 0014 exclude. There is no GraphQL WebSocket protocol (P1 to P3).
2. **Request formats:** POST JSON and GET query string as in GraphQL over HTTP (sections 4.3 and 4.4). The GET form exists for `EventSource`. JSON queries over GET are not added (P4, P18).
3. **Schema:** `events(types, lastEventId): Event!` with `Event { id, type, data: JSON }`, the WebSocket frame's object (P5).
4. **Wire:** `next` per result, `complete` with a bare `data:` after a single result, `id:` on subscription results, the keep-alive comment, no `retry:`. Bytes are written by hand because axum's SSE type cannot write a bare `data:` (P6, P7, P13, P19).
5. **Errors and limits:** validation inside the accepted stream, as the protocol requires; availability as the 503 problem before the stream, which graphql-sse clients retry; one root field per subscription, enforced by the service as the direct-field rule; `Event.data` costing more than half of the complexity limit, so one result holds one copy of the event object (P9 to P11, P18).
6. **The head after the decision:** the transport drives the operation until it registered, was refused or produced its first result, inside the request budget. A per-request link carries the phase and each result's id. A subscription takes its first step only after the head is decided (P12).
7. **Ends:** `complete` only for single results. A shutdown or a failed replay ends a subscription without `complete`, so clients resume (P15).
8. **Caching and CORS:** `Vary: accept` on every GET answer, set outside the request budget; the CORS allow-list unchanged (P17, P20).
9. **Amendment:** it amends the sentence "with no subscription" of ADR 0007 (`docs/adr/0007-api-contracts.md:15`). ADR 0007, a template record, is not rewritten.

**ADR 0022, "Every route in the OpenAPI document"** (`docs/adr/0022-every-route-in-the-openapi-document.md`), same form. It records:

1. **Scope:** every route the service serves is an operation of the document, the documentation routes and `/ping` included (maintainer decision of 2026-11-17); it amends ADR 0006 (`/ping` outside the OpenAPI router) and ADR 0007, and supersedes bundle 4's choice to keep the streaming routes out.
2. **One registration:** every route goes through `OpenApiRouter::routes(routes!(..))`; three routers by state, merged; the asset route at one segment.
3. **Streaming operations:** the event schema of one event (the OpenAPI 3.2 convention), the 101 of the WebSocket route with its handshake headers and its frames in the description.
4. **The GET rule of the negotiation:** a wildcard admits the event stream on GET; a missing `Accept` keeps the 405.
5. **Completeness tests:** the source oracle, the exact operation set, every operation served.
6. **Schemathesis:** the operations tested with the protected flags; a failure fixed at its root; an exclusion only as a maintainer's decision after an escalation, never by a story. The one such decision excludes `GET /api/events/ws` (`71aab46`), whose valid 101 Schemathesis reports as a failure; the operation stays on the Swagger page, and the other ten are tested.

## Protected Changes

The rust-implementer agent never edits a protected path (`protected-config.toml`), and the plugin refuses a story commit that changes one. Each change below proved necessary after planning, was approved by the maintainer under the label `approved-config-change`, and was committed as its own commit outside the stories. No other protected file changes for issue #8.

| File | Exact change | Commit | Approved | Why |
| --- | --- | --- | --- | --- |
| `.spectral.yaml` | six lines: an override that turns the built-in rule `operation-success-response` off for `GET /api/events/ws` only; every other rule keeps applying to it | `a374895` | 2026-11-17 | the WebSocket route answers `101 Switching Protocols`, and the rule asks every operation for a 2xx or 3xx answer (story 4.4) |
| `make/gates.mk` | one line: `SCHEMATHESIS_FLAGS` gains `--exclude-operation-id api_events_ws_get` | `71aab46` | 2026-11-17 | Schemathesis 4.33.1 reports the valid handshake's `101` as a failure (ST101, ST202) and no change to the service can give it another answer; the operation stays documented on the Swagger page |
| `deny.toml` | two entries in `[advisories] ignore`: `{ id = "RUSTSEC-2026-0542", reason = "humantime is unmaintained, not vulnerable, and has no safe upgrade; it remains only through cucumber 0.23.0 (the newest release), a dev-dependency of the end-to-end tests. Remove when cucumber drops it." }` and `{ id = "RUSTSEC-2026-0543", reason = "chrono is unmaintained, not vulnerable, and has no safe upgrade; it enters only through the test-util feature of aws-smithy-http-client, enabled by the dev-dependency aws-smithy-mocks, so the production binary never compiles it. Remove with SQS." }` | `c4f83b3` | 2026-11-20 | the two advisories, published after 2026-11-17, made `make.check_security` fail on `main` and on this branch; neither crate is vulnerable or has a safe upgrade; after story 1.4 both are reached only through dev edges (NFR-21, M4) |
| `tests/CLI/bats/make_quality_gates.bats` | two lines of test G8 ("unused-dependencies names an unused dependency and leaves Cargo.lock as it found it"): the dependency it plants into `xtask/Cargo.toml` becomes `jiff.workspace = true` instead of `humantime.workspace = true`, and its assertion becomes `assert_output --partial 'jiff'` instead of `assert_output --partial 'humantime'` | `49641e7` | 2026-11-20 | story 1.4 removes `humantime` from `[workspace.dependencies]`, so the old plant made `cargo update --workspace --offline` fail ("dependency.humantime was not found in workspace.dependencies") and the Bats quality group of CI would fail; `humantime` cannot stay in `[workspace.dependencies]`, because `deny.toml` denies an unused workspace dependency (`[bans.workspace-dependencies] unused = "deny"`); a replay of G8 in a scratch copy with story 1.4 gave `cargo update` exit 0, `cargo shear --locked` exit 1 naming `jiff`, and `Cargo.lock` unchanged |

Story 5.1 checks this list against the protected files that `git diff --name-only c36f708..HEAD` names. `make.ci_preflight` runs with `CONFIG_CHANGE_APPROVED=true` only while pull request #9 carries `approved-config-change` for these changes and for the three lines of issue #7.

## Build Order

The epics stage cuts the stories. This order keeps each step testable with the DB-free targets until the broker tests:

1. **Event-stream application crate:** `dto.rs` cursor refactor; `StreamPhase`, `StreamLink`, `EventObject`, `EventStreamSubscription` with their unit tests. Depends on nothing.
2. **Schema and contract:** the `Subscription` root, the SDL oracles, the regenerated snapshot, `make.graphql_diff`. Depends on 1.
3. **Pure transport modules:** `negotiation.rs`, `query_string.rs`, `single_root.rs` with their tables; the `graphql.rs` declarations; the `.templatesyncignore` entry for `graphql.rs`. Depends on nothing, so it runs in parallel with 1.
4. **Transport and wiring:** `event_stream.rs` (first without, then with the keep-alive timer), the handlers, `vary.rs`, the router, the bootstrap line of `QueryStringLimit`, the manifest; the router, CORS and oracle tests; the `.templatesyncignore` entry for `handler.rs`. Depends on 1, 2 and 3.
5. **Broker integration and end-to-end tests** (serialized verify step). Depends on 4.
6. **K6 subscriber, documentation, ADR 0021, `workspace.dsl`.** Depends on 4.
7. **Verification** of every local gate of `make ci` (NFR-15). Depends on 5, 6, 8 and 9.
8. **OpenAPI document:** first the streaming routes of the event-stream crate (annotations, parameters, `routes!`), then the GraphQL, liveness and documentation routes, the merge of the three routers, the completeness tests, the snapshot, ADR 0022 and the Schemathesis run. Depends on 1 and on the GET form of 4.
9. **Durations and timestamps without humantime** (added on 2026-11-20): the manifests and the lock first, then `config.rs` with jiff, the envelope's calendar rule and `docs/environment.md` (see "Durations and Timestamps without humantime"). Depends on nothing that is not done: it shares only `crates/service/Cargo.toml` with step 4, which is done, so it runs next to steps 5 to 8.

## Failure Modes

| Failure | Effect | Handling |
| --- | --- | --- |
| A client sends two root fields, a root fragment or a root `@skip` | it would register two subscribers or none | the root rule refuses it with `VALIDATION_FAILED` before any resolver runs |
| A client selects `data` under many aliases or fragments | each result would copy the event object once per selection, per event and subscriber | `Event.data` costs 501 of the complexity limit 1 000, so a second selection is a document error before any subscriber registers |
| A client's `Accept` lists the JSON types and the event stream at the same q (urql queries) | it would get a stream it does not expect | the tie goes to JSON (FR-2) |
| The node is starting, full or shutting down | the subscription cannot be served | 503 before the head; graphql-sse clients retry with backoff, possibly on another node |
| The broker is away while a cursor is below the window | the replay cannot read | the head and keep-alives are written; the stream ends without `complete` at the first failed read; `replay failed` is logged |
| A GraphQL client reads slowly | its queue fills | it falls behind and catches up on its own connection; others are unaffected |
| A GraphQL client accepts no data | the socket stalls | `TCP_USER_TIMEOUT` ends it; the slot is freed |
| The budget expires before the head (a slow query) | no stream | the 503 problem; the dropped future frees any subscriber |
| A cache stored a GET answer | an `EventSource` could get the stored page | `Vary: accept` on every GET answer; `EventSource` uses no-store anyway |
| Shutdown during a stream | the stream ends | after the queued results, without `complete`; the client resumes elsewhere with its last id |
| A wiring fault (settings, link or state missing) | the transport cannot run | a 500 problem or an `INTERNAL_SERVER_ERROR` result, logged once; covered by unit tests |
| A route is added with a plain `.route(..)` | it is served without an entry | the source oracle of `openapi/tests.rs` fails, naming the file and line |
| Schemathesis refuses a streaming operation | `make.schemathesis_validate` fails | fixed at its root in the document or the code; an unfixable case is escalated with the exact exclusion, never excluded by the plan |
| A RustSec advisory marks a crate unmaintained | `make.check_security` fails | a crate of the production build is replaced (story 1.4 for humantime); a crate that only dev edges reach is ignored in `deny.toml` with its reason and removal condition, only as a protected change the maintainer approved (NFR-21) |
| A duration or `occurredAt` value would read differently after humantime leaves | a node would accept a configuration or an event it refused, or the reverse | the tests of story 1.4 pin every documented value, the moved duration edges and every edge of the time rule |
