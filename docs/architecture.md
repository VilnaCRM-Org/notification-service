# Architecture overview

This document describes how the service is built, why the code is arranged the way it is, and which assumptions about the deployment the design relies on. It is written for a reviewer who has not read the planning bundle. The reasons behind each structural choice are in the [decision records](adr/README.md); the C4 model of the system is in [`workspace.dsl`](../workspace.dsl) at the repository root and is served by the Structurizr viewer of the development environment on `http://localhost:8080`.

## What the service is

One Rust service binary, `service`, that gives a new VilnaCRM service the platform baseline of the PHP template. It serves REST under `/api` with a code-first OpenAPI 3.1.0 document, a GraphQL endpoint at `/api/graphql`, a readiness probe at `GET /api/health`, a liveness probe at `GET /ping` and problem documents (RFC 9457) for every error. A small example bounded context shows how a use case, an aggregate, a domain event, a repository and a PostgreSQL adapter fit together.

## Layers

Every bounded context is split into three layers, each its own crate. Cargo enforces the direction of the dependencies, because a crate can only use what its manifest lists, and `cargo xtask arch` enforces the rest ([ADR 0001](adr/0001-workspace-and-layers.md), [ADR 0002](adr/0002-architecture-gate.md)).

| Layer | What belongs in it | May depend on |
| --- | --- | --- |
| Domain | Aggregates, value objects, domain events, domain errors, repository ports and every other port that an adapter implements (`Cache`, `Clock`, `UuidGenerator`, the three bus ports, `HealthProbe`). No framework. Only `thiserror`, `uuid` and `async-trait` are allowed from outside the workspace. | Domain crates only |
| Application | Use cases (commands, queries, handlers, event subscribers, application errors) and the API-facing adapters (HTTP handlers, request and response types, OpenAPI annotations, GraphQL types and resolvers). The use-case modules do not import `axum`, `utoipa` or `async-graphql`; the `http` and `graphql` modules do. | Domain and Application crates |
| Infrastructure | Driven adapters: SQL and row mapping, the in-process buses, the cache, the clock, the UUID generator, the SQS client factory, the health probes. | Domain and Infrastructure crates |

Application and Infrastructure never depend on each other. A handler therefore cannot run SQL, and an adapter cannot call a use case. Only the composition root, the `service` crate, depends on all three layers and is where adapters meet use cases (`service::bootstrap::Adapters`).

## Crates and bounded contexts

The workspace has thirteen crates: nine layer crates (three bounded contexts times three layers), the composition root `service` (a library plus the binary), the test-double crate `test-support`, the Gherkin suite `e2e-tests` and the tooling crate `xtask`.

| Bounded context | Purpose | Crates |
| --- | --- | --- |
| `shared` | The shared kernel: aggregate root, domain event, bus ports and their in-process adapters, technical ports and adapters, the problem type and the problem-aware extractors | `shared-domain`, `shared-application`, `shared-infrastructure` |
| `health-check` | The readiness check: the probe port, the orchestration of the three probes, and the PostgreSQL, cache and SQS probes | `health-check-domain`, `health-check-application`, `health-check-infrastructure` |
| `some-module` | The example context: create, read by id and list `ExampleApiResource`, REST and GraphQL | `some-module-domain`, `some-module-application`, `some-module-infrastructure` |

A crate belongs to the context named by the text its layer pattern matches in `architecture.toml` (`some-module-domain` belongs to `some-module`). A context may use itself and the shared kernel and nothing else, so adding a context changes no rule file. `cargo xtask new-context <group> <context>` creates the three crates and registers them in the workspace. The layer crates live at `crates/<group>/<context>/<layer>`; the shared kernel lives at `crates/shared/<layer>`; `service` is at `crates/service`, `test-support` at `tests/support/`, `e2e-tests` at `tests/e2e/` and `xtask` at `xtask/`.

## Request flow

A request to the REST API passes through these steps, from the socket to the database and back.

1. The router assembled by `service::http::router::assemble` receives it. Eight layers wrap every route, outermost first: sanitising of an inbound `x-request-id`, generation of the id, echo of the id on the response, the four security headers, the request span and access log, the panic catcher, the request time budget and the body limit. Because these wrap the fallbacks too, a 404, a 405, a timeout and a panic all carry the request id and the security headers and are logged.
2. The route matches a handler in the `http` module of an Application crate. The problem-aware extractors parse the path, query and JSON body and validate them. A request that cannot be parsed or validated is answered here with a problem document.
3. The handler builds a command or a query with a UUID that it generated through the `UuidGenerator` port, and dispatches it through the command bus or the query bus. The buses are ports defined in the shared Domain crate and implemented in memory in the shared Infrastructure crate. Handlers are registered explicitly in `service::bootstrap`. The health endpoint is the one handler that calls its service directly, so that a health check never depends on bus registration.
4. The use-case handler works on the aggregate and talks to the repository port. The PostgreSQL adapter in an Infrastructure crate implements the port with one checked SQL statement per method. A command changes one aggregate and returns no data.
5. After a successful save the handler publishes the domain events on the event bus. Subscribers run synchronously, in registration order, and a failing subscriber does not stop the others.
6. The handler maps the result to a response. The GraphQL route follows the same path through the same commands and queries.

The OpenAPI document and the GraphQL schema are built from the same route and type definitions that serve traffic, without any adapter, so both can be exported with no environment, database or network ([ADR 0007](adr/0007-api-contracts.md)).

## Error model

Errors have four levels, and each level converts to the next in exactly one place ([ADR 0003](adr/0003-error-model.md)):

- Domain errors are `thiserror` enums per concept and know nothing about HTTP.
- Application errors are one enum per bounded context and are the error type of that context's commands and queries.
- Bus errors wrap the application error, or report a missing handler.
- The problem is the HTTP representation: `application/problem+json` with `type`, `title`, `status` and, where it helps, `detail` and an extension member. Every 5xx problem is created in one function that logs the error chain once and returns a body without internal detail.

One table fixes the status codes (400, 404, 405, 413, 415, 422, 500 and 503) and the two problem types that carry extension members: a validation failure lists its `violations`, and a failed health check lists its `failures`. GraphQL operation errors are not problem documents; they are returned in `errors[]` with a stable `extensions.code`.

## Operational assumptions

The design relies on the following assumptions about how the service is run. They are properties of the template, not options.

- **No session or per-client state.** The service keeps no session and no per-client state between requests. Any instance can answer any request, and a resource created through one instance can be read through another. A test reads a resource through an application instance other than the one that created it.
- **PostgreSQL is the only source of truth.** The in-process cache sits behind a port and is used only by the cache health probe. No endpoint response depends on the content of the cache, so an instance restart or a cold cache changes no answer. A remote cache is an extension through the same port.
- **Domain-event delivery is at most once.** Events are published after the save succeeded and are not part of a database transaction, and no outbox ships. If a subscriber fails after the save, the request still succeeds with `201`, the failure is logged once per subscriber, and the event is not retried. A service that needs guaranteed delivery adds an outbox (see [extension seams](extension-seams.md)).
- **A proxy or load balancer sits in front of the service.** The service speaks plain HTTP. A proxy or load balancer terminates TLS, adds `Strict-Transport-Security` and cross-origin headers where needed, and limits header-read time and the number of connections, because the server has no header-read timeout and no connection limit of its own. Per-request time and body size are limited by the service itself.
- **Shutdown is bounded.** On SIGTERM or SIGINT the service stops accepting connections and drains in-flight requests for at most `SHUTDOWN_GRACE_PERIOD`. It exits with status 0 when all finished and with status 1 when the period ended with a request still open, so a cut request is visible to the platform.
- **Start-up does not wait for dependencies.** The database pool is lazy, so `serve` starts and answers `/ping` while the database is down, and `/api/health` reports the failing dependency. Migrations are a separate step, never run by `serve`.

## Quality gates and the decision records

The architecture is held in place by gates that fail: layer and context isolation, a lint policy with complexity limits, 100% line and function coverage, mutation testing, contract snapshots and a protected-configuration guard. Run them together with `make ci`; the verification section of `AGENTS.md` lists them in order, and [`docs/make-targets.md`](make-targets.md) describes each one. The decisions behind the structure are written down as twelve records, indexed in the [decision record index](adr/README.md).
