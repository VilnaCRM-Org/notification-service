# Extension seams

The template ships what its requirements ask for and nothing more. Four features that a service often needs are left out on purpose, and each has one named place where it plugs in. This document says where that place is, what to add and what to leave alone. The examples name the crates and files of the template; adapt them to the names of your service.

Most seams change files that the template sync keeps in step with the template. Read [Seams and the template sync](#seams-and-the-template-sync) before you change one: each seam lists the synchronised files it changes, and each of them goes into your `.templatesyncignore` in the same pull request.

## Contents

- [Redis adapter through the `Cache` port](#redis-adapter-through-the-cache-port)
- [Tracing and metrics at the request span](#tracing-and-metrics-at-the-request-span)
- [Authentication layer](#authentication-layer)
- [Outbox for domain events](#outbox-for-domain-events)
- [Seams and the template sync](#seams-and-the-template-sync)

## Redis adapter through the `Cache` port

The port is `shared_domain::port::Cache`:

```rust
#[async_trait::async_trait]
pub trait Cache: std::fmt::Debug + Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: std::time::Duration) -> Result<(), CacheError>;
}
```

The shipped adapter is the in-process `shared_infrastructure::cache::MokaCache`, bounded to 10 000 entries. Its only user is the cache health probe, and no response of the service depends on cache content (the service is stateless, so several instances can run behind a load balancer). A remote cache is an adapter for the same port:

1. Add the Redis client crate to `crates/shared/infrastructure/Cargo.toml` and to the allow-list of that layer in `architecture.toml`. The change to `architecture.toml` is protected configuration and needs the label `approved-config-change`.
2. Add `shared_infrastructure::cache::RedisCache` next to `MokaCache`. Prefix it with its technology, implement `Cache` for it, and give it a `Debug` implementation that never prints keys or values. Map every client error to `CacheError`. Set connection and operation timeouts below `HEALTH_PROBE_TIMEOUT` (default 3 seconds), because the probe is wrapped in that deadline.
3. Add the settings (URL, timeouts) to `ServeArgs` and `Config` in the `service` crate, with their validation. Every setting is an environment variable: add each to `.env`, to `.env.test` where tests need it, and to `docs/environment.md`, which `make docs-check` compares with the code. Never log or echo the URL if it holds a password.
4. Construct the adapter in `Adapters::connect` and hand it to `CacheProbe` instead of `MokaCache`. The handler of `/api/health` and `HealthService` do not change.
5. Add a Redis container to `docker-compose.yml` and its variables, then add an integration test of the adapter against it, next to `integration/cache.rs`.

Synchronised files this seam changes: `crates/shared/infrastructure/Cargo.toml`, `crates/shared/infrastructure/src/cache.rs` and its unit tests `crates/shared/infrastructure/src/cache/tests.rs` (or `crates/shared/infrastructure/src/lib.rs` when the adapter gets a module of its own), `docker-compose.yml` and `crates/shared/infrastructure/tests/integration/main.rs`. The other files of the steps are already on the template's ignore list (the workspace `Cargo.toml` and `crates/service/Cargo.toml`, `architecture.toml`, the configuration, command-line and bootstrap modules of `service`, `.env`, `.env.test`, `docs/environment.md`, `crates/internal/health-check/infrastructure/src/probe.rs`). A probe of your own whose module `probe.rs` declares and that needs a client crate the health-check infrastructure crate does not depend on yet (a probe that talks to Redis directly instead of through `CacheProbe`, for example) also changes `crates/internal/health-check/infrastructure/Cargo.toml`. Unlike `probe.rs`, that manifest is synchronised: list it by its full path in your `.templatesyncignore` in the same pull request, or the next sync removes the dependency.

The probe stores the key `health_check` with the value `ok` for 60 seconds, reads it back and compares. With a remote adapter this probe becomes meaningful: a failure reports `Cache is not working` (see the runbook in `docs/operations.md`). Using the cache from a use case is a second step: take `Arc<dyn Cache>` in the handler and add a field to `Adapters`, which the template does not have because nothing but the probe used one. A use case that reads the cache must still produce a correct response when the cache is empty.

## Tracing and metrics at the request span

Every request runs inside the span `http.request`, created by `service::http::trace::make_span` through `TraceLayer` in the middleware stack, with the fields `http.request.method`, `http.route` (the route template, or `unmatched`), `url.path` and `request_id`. The access log line `request completed` is written when the response is ready. This span is the seam for exporters:

- Traces. Add `tracing-opentelemetry` and an OTLP exporter, and add the OpenTelemetry layer to the subscriber that `service::telemetry::init` builds, next to the JSON formatting layer. The span already carries the attribute names of the OpenTelemetry HTTP conventions, so no request code changes. The request id is a field of the span; keep it, so a trace and a log line of the same request can be joined.
- Metrics. Add a metrics layer to the middleware stack next to `TraceLayer`, or record a histogram of `duration_ms` and a counter per `http.response.status_code` in `service::http::trace::on_response`, labelled with `http.route` and `http.request.method` (never with `url.path` or the request id, whose values are unbounded). Expose them through a Prometheus endpoint outside `/api` and outside the OpenAPI router, as `/ping` is, or push them through the exporter.
- Settings. Endpoint, protocol and sampling are configuration: add them as environment variables as in step 3 above, with defaults that disable the exporter, so that development and the test suites need no collector.

Synchronised files this seam changes: `crates/service/src/telemetry.rs`, `crates/service/src/http/trace.rs`, `crates/service/src/http/router.rs` when a metrics layer joins the stack, and `crates/service/src/command/serve.rs` for the flush at shutdown. The settings go into the configuration modules, which the template already ignores.

Keep these rules when you add an exporter. `serve` must keep writing only JSON lines on stdout (the exporter must not print to stdout or stderr), the shutdown sequence must flush the exporter inside the grace period, and no request or response body or header other than the request id may become a span attribute.

## Authentication layer

The template has no authentication: every route is public. The seam is the router in `service::http::router`, where the middleware stack is built (see the layer table in `docs/architecture.md`). To add authentication:

1. Write the layer as middleware in `service::http`, for example a function used with `axum::middleware::from_fn_with_state`, which validates the credential and puts the caller's identity into the request extensions. Use `tower_http::validate_request` or your identity provider's library for the validation itself.
2. Place the layer inside the layers that set and propagate the request id, the security headers and `TraceLayer`, and outside the handlers. A rejected request then still carries the request id and the security headers and is logged. Answer a rejection with a problem document of the error model (`401` or `403`, `application/problem+json`) so that clients see one error format.
3. Leave `/ping` and `/api/health` outside the protected routes: the container healthcheck and the load balancer call them without credentials.
4. Mark the credential header as sensitive (`SetSensitiveRequestHeadersLayer` of `tower-http`, whose `sensitive-headers` feature the template does not enable) before the span is created. The span records no header today, which is why the template needs no such layer; keep it that way.
5. Describe the scheme in the OpenAPI document (`securitySchemes` and a `security` requirement on the protected operations), regenerate the snapshots with `make generate-openapi-spec`, and review the diff: adding a requirement is a breaking change for existing clients and needs the label `approved-breaking-change`. The GraphQL route is a route of the same router and is protected by the same layer; pass the identity from the request extensions to the schema as context data when a resolver needs it.
6. Add scenarios for the missing and the invalid credential in a feature file of your own, `features/<name>.feature`, and update the Schemathesis run, which must then send credentials. Do not add them to `features/health_check.feature`: it stays identical to the PHP template's file (FR55(a)), and its four scenarios keep passing because step 3 leaves `/api/health` outside the protected routes.

Synchronised files this seam changes: `crates/service/src/http/router.rs`, and `crates/service/src/graphql/handler.rs` when a resolver needs the identity (its `execute` passes the request to the schema). The middleware itself and the feature file of step 6 are new files of your own, which the sync never touches; the module declaration of the middleware goes into `crates/service/src/http.rs`, the OpenAPI changes into the OpenAPI module and the snapshots, all of which the template already ignores. `features/health_check.feature` is not among the files this seam changes: it changes only together with the PHP template, and the test "FR55(a): features/health_check.feature is the PHP template file" of `make_general.bats` fails when its content differs from the PHP file. The `Makefile` and the workflows are synchronised too: give Schemathesis its credentials through `SCHEMATHESIS_FLAGS` instead of changing the recipe.

Rate limiting, CORS and similar edge concerns belong to the same stack and follow the same pattern: one layer, placed deliberately in the order, with a test.

## Outbox for domain events

Domain-event delivery is at most once. A command handler saves the aggregate and then publishes its events on the in-process event bus. When a subscriber fails after the save succeeded, the handler logs `event publication failed` at ERROR, still returns success and the client receives the success response. The event is lost for that subscriber. The only shipped subscriber writes the audit log line, for which this is acceptable. A service whose subscribers must not miss an event needs a transactional outbox.

The seam is the persistence design: each repository method is one statement and one command changes one aggregate, and events are not part of a database transaction. To add an outbox:

1. Add a migration pair that creates an `outbox` table in the context that owns the events (`make create-migration name=create_outbox`; see `docs/operations.md` for the migration rules): event id, event name, aggregate id, serialised payload, `occurred_at`, a state or `published_at` column, and an index for the unpublished rows.
2. Introduce a unit-of-work port in the Domain crate of the context, with an adapter in its Infrastructure crate that saves the aggregate and inserts the outbox rows in one SQL transaction. The handler calls the unit of work instead of the repository and the bus.
3. Add a relay: a task that reads unpublished rows in order, publishes them to the event bus or to a queue (the SQS client of `shared_infrastructure::messaging::sqs_client` is the seam for an asynchronous consumer), and marks them as published. Delivery then becomes at least once, so every subscriber must be idempotent, which the guidance already demands.
4. Start the relay from `serve` and stop it inside the shutdown grace period. Add its failures to the log messages in `docs/operations.md`.
5. Regenerate `.sqlx/` with `make sqlx-prepare`, add integration tests that kill the relay between the insert and the mark, and remove the statement about at-most-once delivery from the documents that carry it.

Synchronised files this seam changes: `crates/service/src/command/serve.rs` (the relay task), `docs/operations.md` (its log messages), and the documents that carry the statement about at-most-once delivery of step 5: `docs/architecture.md`, `.claude/skills/api-resource-development/SKILL.md` and this guide, `docs/extension-seams.md`. The migration, the context crates and `.sqlx/` are already service-owned.

## Seams and the template sync

The template sync (see the sync guide, `.github/TEMPLATE_SYNC_APP.md` or `.github/TEMPLATE_SYNC_PAT.md`) resolves every difference in favour of the template, so a sync pull request removes what a service added to a synchronised file. The template does not put the files of these seams on its own ignore list on purpose: a fix of the router, the request span or the shutdown sequence must keep reaching every service that has not changed them. A service that uses a seam therefore:

1. Adds every synchronised path that the seam changes, as listed under the seam, to its own `.templatesyncignore` in the same pull request as the change. A listed production file whose unit tests live in a sibling `tests.rs` (for example `crates/service/src/graphql/handler/tests.rs` next to `handler.rs`) changes with its tests, so that `tests.rs` is ignored with it. Each line of that file is a git pathspec from the root of the repository, not a `.gitignore` pattern: a bare name such as `Cargo.toml` is the root file only, so write every path in full (the sync guide describes the format). Check a path with `sed 's/#.*//' .templatesyncignore | xargs -r git ls-files -- | grep -Fx -- <path>`: it prints the path once it is ignored.
2. Applies later template changes to these files by hand, as the section "Apply an ignored change by hand" of the sync guide describes.
3. Removes a path from its `.templatesyncignore` again when it drops its change to that file, so that the template's version arrives.
