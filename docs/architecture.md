# Architecture overview

This document describes how the service is built, why the code is arranged the way it is, and which assumptions about the deployment the design relies on. It is written for a reviewer who has not read the planning bundle. The reasons behind each structural choice are in the [decision records](adr/README.md); the C4 model of the system is in [`workspace.dsl`](../workspace.dsl) at the repository root and is served by the Structurizr viewer of the development environment on `http://localhost:8080`.

## What the service is

One Rust service binary, `service`, built on the platform baseline of the VilnaCRM Rust service template. It is the real-time event delivery hub of VilnaCRM: it reads the events that other services write to one Kafka topic and delivers them to frontends over Server-Sent Events and WebSocket, an open-source alternative to Mercure. It serves REST under `/api` with a code-first OpenAPI 3.1.0 document, a GraphQL endpoint at `/api/graphql`, the two streaming routes `GET /api/events/stream` and `GET /api/events/ws` (specified in [`streaming-protocol.md`](streaming-protocol.md), outside the OpenAPI document), a readiness probe at `GET /api/health`, a liveness probe at `GET /ping` and problem documents (RFC 9457) for every error. The service stores nothing and has no database: the template's PostgreSQL adapter and its example bounded context were removed ([ADR 0020](adr/0020-postgresql-is-removed.md)).

## Layers

Every bounded context is split into three layers, each its own crate. Cargo enforces the direction of the dependencies, because a crate can only use what its manifest lists, and `cargo xtask arch` enforces the rest ([ADR 0001](adr/0001-workspace-and-layers.md), [ADR 0002](adr/0002-architecture-gate.md)).

| Layer | What belongs in it | May depend on |
| --- | --- | --- |
| Domain | Aggregates, value objects, domain events, domain errors, repository ports and every other port that an adapter implements (`Cache`, `Clock`, `UuidGenerator`, the three bus ports, `HealthProbe`, the three topic ports `TopicProbe`, `TopicTail` and `TopicReplay`). No framework. Only `thiserror`, `uuid` and `async-trait` are allowed from outside the workspace. | Domain crates only |
| Application | Use cases (commands, queries, handlers, event subscribers, application errors) and the API-facing adapters (HTTP handlers, request and response types, OpenAPI annotations, GraphQL types and resolvers). The use-case modules do not import `axum`, `utoipa` or `async-graphql`; the `http` and `graphql` modules do. | Domain and Application crates |
| Infrastructure | Driven adapters: the in-process buses, the cache, the clock, the UUID generator, the SQS client factory, the health probes, the Kafka reader of the event stream. | Domain and Infrastructure crates |

Application and Infrastructure never depend on each other. A handler therefore cannot call Kafka or another external system, and an adapter cannot call a use case. Only the composition root, the `service` crate, depends on all three layers and is where adapters meet use cases (`service::bootstrap::Adapters`).

## Crates and bounded contexts

The workspace has thirteen crates: nine layer crates (three bounded contexts times three layers), the composition root `service` (a library plus the binary), the test-double crate `test-support`, the Gherkin suite `e2e-tests` and the tooling crate `xtask`.

| Bounded context | Purpose | Crates |
| --- | --- | --- |
| `shared` | The shared kernel: aggregate root, domain event, bus ports and their in-process adapters, technical ports and adapters, the problem type and the problem-aware extractors | `shared-domain`, `shared-application`, `shared-infrastructure` |
| `health-check` | The readiness check: the probe port, the orchestration of the probes, and the cache and SQS probes (the probe of the event stream lives in the composition root, because the two contexts may not depend on each other) | `health-check-domain`, `health-check-application`, `health-check-infrastructure` |
| `event-stream` | The event delivery hub: the catalog of event types (`EVENT_TYPES`), the tail of the Kafka topic, the memory window, the hub that fans events out to subscribers, the subscriptions with resume and replay, the SSE and WebSocket routes, the node status, and the Kafka adapter behind the topic ports (see [The event stream](#the-event-stream)) | `event-stream-domain`, `event-stream-application`, `event-stream-infrastructure` |

A crate belongs to the context named by the text its layer pattern matches in `architecture.toml` (`event-stream-domain` belongs to `event-stream`). A context may use itself and the shared kernel and nothing else, so adding a context changes no rule file. `cargo xtask new-context <group> <context>` creates the three crates and registers them in the workspace. The layer crates live at `crates/<group>/<context>/<layer>`; the shared kernel lives at `crates/shared/<layer>`; `service` is at `crates/service`, `test-support` at `tests/support/`, `e2e-tests` at `tests/e2e/` and `xtask` at `xtask/`.

## Request flow

A request to the REST API passes through these steps, from the socket to the use case and back.

1. The router assembled by `service::http::router::assemble` receives it. These layers wrap every route, outermost first: the three request-id layers (sanitising of an inbound `x-request-id`, generation of the id, echo of the id on the response), the four security headers, the request span and access log, the CORS layer of `CORS_ALLOWED_ORIGINS` (present only when the variable allows an origin), the origin check of a WebSocket handshake, the panic catcher, the request time budget and the body limit. Because these wrap the fallbacks too, a 404, a 405, a timeout and a panic all carry the request id and the security headers and are logged.
2. The route matches a handler in the `http` module of an Application crate. The problem-aware extractors parse the path, query and JSON body and validate them. A request that cannot be parsed or validated is answered here with a problem document.
3. The handler builds a query and dispatches it through the query bus; a context that changes state builds a command, with a UUID that it generated through the `UuidGenerator` port, and dispatches it through the command bus. The buses are ports defined in the shared Domain crate and implemented in memory in the shared Infrastructure crate. Handlers are registered explicitly in `service::bootstrap`. This service registers one, the query of the event-type catalog (`GET /api/event-types`), and wires no command bus, because none of its use cases changes state. The health endpoint and the status route of the event stream call their services directly, so that neither depends on bus registration.
4. The use-case handler works through the ports of its Domain crate, which adapters in an Infrastructure crate implement. A command changes one aggregate and returns no data.
5. A command handler publishes the domain events of a successful change on the event bus. Subscribers run synchronously, in registration order, and a failing subscriber does not stop the others. No handler of this service publishes a domain event, and no subscriber is registered.
6. The handler maps the result to a response. The GraphQL route follows the same path through the same queries.

The OpenAPI document and the GraphQL schema are built from the same route and type definitions that serve traffic, without any adapter, so both can be exported with no environment or network ([ADR 0007](adr/0007-api-contracts.md)).

## The event stream

The bounded context `notification/event-stream` delivers the events of one Kafka topic to the frontends. Producers, the other VilnaCRM services, write each event as a JSON envelope (`id`, `type`, `occurredAt`, `data`) to the topic `KAFKA_TOPIC`, which has exactly one partition. The service never writes to Kafka. The wire protocol of the two streaming routes is specified in [`streaming-protocol.md`](streaming-protocol.md); running a node is described in [`operations.md`](operations.md#event-stream).

```text
producers --> Kafka topic (1 partition) --+--> node A: tail --> window (memory) --> hub --> queue per subscriber --> SSE / WebSocket
                                          +--> node B: tail --> window --> hub --> ...
                                          +--> any node: replay from Kafka <-- subscription below its window
```

**Every node reads the whole topic.** Each node runs one supervised background loop, the tail (`TailEvents`), that reads the topic from Kafka without a consumer group: it joins no group, commits no offset and keeps its position in memory only ([ADR 0013](adr/0013-every-node-reads-kafka-directly.md)). It reads with read-committed isolation, so a record of an aborted transaction never reaches a client. At start a node reads the watermarks of the topic, starts `EVENT_WINDOW_START_EVENTS` offsets below the end and becomes ready the first time its position reaches the end; until then it refuses subscriptions with a 503, so no subscription can miss an event written around its start. A node reads the watermarks at least once per second on a connection of its own, because a long poll of the Kafka client reports no broker outage. Every broker call is bounded by its own time-out, a failed round is retried after `KAFKA_RETRY_DELAY` from the same position, and a record that is not a valid envelope is skipped, logged and counted without stopping the tail.

**No leader, no election, no link between nodes.** Nothing needs a single writer: the service writes nothing, and the cursor of an event, its Kafka offset plus 1, is the same on every node ([ADR 0014](adr/0014-no-election-lock-or-link-between-nodes.md), [ADR 0015](adr/0015-one-topic-one-partition.md)). Nodes share no state besides the topic and know nothing about each other, so a client resumes on any node and a node can be added, restarted or lost without touching the others. The cost is that the broker serves the whole topic once per node and every node validates every event: adding nodes adds room for subscribers, not event rate.

**The window.** Each node keeps the valid events its tail read, in id order, in a memory window bounded by `EVENT_WINDOW_MAX_EVENTS` events and `EVENT_WINDOW_MAX_BYTES` bytes; the oldest events leave first. A resume whose cursor is inside the window is served from memory and reads nothing from Kafka.

**The hub.** The tail pushes every event to the window first and then publishes it to the hub. The hub keeps, per event type that has a subscriber, the queues of its subscribers. It encodes an event once per transport (the SSE frame and the WebSocket text frame) and hands one shared copy to each queue of a subscriber of that type, with a send that never waits: the cost of an event grows with the number of subscribers of its type, never with the number of connections, and a slow client never delays the tail or the other clients. A subscriber whose queue of `SUBSCRIBER_BUFFER` events is full is not disconnected; it receives its queued frames, then what it missed from the window or Kafka, then live events again. The hub also enforces `MAX_SUBSCRIBERS` per node and answers the 503s while the node is starting or shutting down.

**Subscriptions, resume and replay.** A subscription is a small state machine (`Subscription`): it resumes from the window when its cursor is inside it, replays from Kafka below it in pages of 500 records under `EVENT_REPLAY_PERMITS` permits per node, each on a Kafka connection of its own, and then follows the hub. Every phase compares offsets with the cursor in the service, so the ids of one connection strictly increase and no event is written twice on it. A replay reaches back at most `EVENT_REPLAY_MAX_EVENTS` offsets below the node's position, and to the topic's retention. Delivery is at least once: the service removes no duplicates, and the client deduplicates by the envelope `id` ([ADR 0017](adr/0017-at-least-once-with-client-side-deduplication.md)).

**The Kafka client behind ports.** The domain crate defines three ports, `TopicProbe` (metadata and watermarks), `TopicTail` (the tail's reader) and `TopicReplay` (pages of older records). In production code only the infrastructure crate uses the Kafka client, `krafka`, pinned to an exact version; the composition root wires the adapter, and its one mention of the client is the log directive `krafka=error`. The service integration tests and the Gherkin suite use the `krafka` producer to write their test records, so a switch to `rdkafka` touches the infrastructure crate, the composition root, the manifests and those tests ([ADR 0016](adr/0016-krafka-pinned-behind-a-port.md)).

**Readiness and status.** The composition root adds a third readiness probe, `event_stream`, after those of the cache and the message broker. It reads no broker: it passes while the node is started and its last watermark read and its last catch-up to the end of the topic are each at most `EVENT_TAIL_STALE_AFTER` old ([ADR 0019](adr/0019-readiness-follows-the-freshness-of-the-tail.md)). `GET /api/events/status` and the GraphQL query `eventStreamStatus` report the node's view of its tail, window and subscribers.

**Shutdown and supervision.** The composition root (`service::background`) runs the tail loop and supervises it: a loop that dies ends the node with exit status 1 after the shutdown sequence, instead of leaving a node that keeps its clients and delivers nothing. At shutdown the node stops the tail, closes the hub so that every stream ends after its queued frames (WebSocket close code 1001), waits at most 2 seconds for the streams, then drains HTTP.

**What is not measured.** No latency, throughput or capacity figure exists for this design. The capacity figure of the plan, 10 000 subscribers on one node of 4 vCPU and 8 GiB with a fan-out below 100 ms (99th percentile) at 100 events per second, is a design target. The change delivers its structural conditions (an event encoded once and shared by the subscribers of its type, a publish path that never waits for a client, one broker read per node for all its clients, bounded memory) and a test that hands one event to 10 000 subscriber queues, which asserts no time. The K6 scenario measures another interval, from the load-test publisher through the broker to a WebSocket client.

## Error model

Errors have four levels, and each level converts to the next in exactly one place ([ADR 0003](adr/0003-error-model.md)):

- Domain errors are `thiserror` enums per concept and know nothing about HTTP.
- Application errors are one enum per bounded context and are the error type of that context's commands and queries.
- Bus errors wrap the application error, or report a missing handler.
- The problem is the HTTP representation: `application/problem+json` with `type`, `title`, `status` and, where it helps, `detail` and an extension member. Every 5xx problem is created in one function that logs the error chain once and returns a body without internal detail.

One table fixes the status codes of the template's routes (400, 404, 405, 413, 415, 422, 500 and 503) and the two problem types that carry extension members: a validation failure lists its `violations`, and a failed health check lists its `failures`. Before a stream opens the streaming routes also answer, as problem documents, 403 for a WebSocket handshake whose `Origin` is not allowed and the 4xx of a rejected WebSocket upgrade (400 or 426); [`streaming-protocol.md`](streaming-protocol.md#answers-before-a-stream-opens) lists every answer of the streaming routes. GraphQL operation errors are not problem documents; they are returned in `errors[]` with a stable `extensions.code`.

## Operational assumptions

The design relies on the following assumptions about how the service is run. They are properties of the template, not options.

- **No session or per-client state.** The service keeps no session and no per-client state between requests and stores nothing, so any instance can answer any request. The event stream holds state of its own only in memory and only for itself: its tail position, its window and the open streams of its clients. A client whose node goes away resumes on any other node with its last event id.
- **Kafka holds all shared state.** The events exist only in the Kafka topic; a node restart loses its window, not an event. The service has no database and writes nothing ([ADR 0020](adr/0020-postgresql-is-removed.md)). The in-process cache sits behind a port and is used only by the cache health probe. No endpoint response depends on the content of the cache, so an instance restart or a cold cache changes no answer. A remote cache is an extension through the same port.
- **A proxy or load balancer sits in front of the service.** The service speaks plain HTTP. A proxy or load balancer terminates TLS, adds `Strict-Transport-Security`, and limits header-read time and the number of connections per client address, because the server has no header-read timeout and no limit per client of its own; the event stream limits only the number of streams per node (`MAX_SUBSCRIBERS`). Cross-origin access is decided by the service itself from `CORS_ALLOWED_ORIGINS`. Per-request time and body size are limited by the service itself; the time limit bounds the time to the response head, so the streams are not cut by it. What the proxy must do for the streams is listed in [`operations.md`](operations.md#the-edge-proxy).
- **Shutdown is bounded.** On SIGTERM or SIGINT the service stops accepting connections, stops the tail loop of the event stream, closes the hub so that every stream ends after its queued events (waiting at most 2 seconds for them), and drains in-flight requests, all within `SHUTDOWN_GRACE_PERIOD`. It exits with status 0 when all finished and with status 1 when the period ended with a request still open or the tail loop died, so a cut request or a dead loop is visible to the platform.
- **Start-up does not wait for dependencies.** The SQS client and the readers of the event topic connect when they are first used, so `serve` starts and answers `/ping` while LocalStack or the Kafka broker is down, and `/api/health` reports the failing dependency. A node that cannot reach its Kafka broker or topic serves every route but the streams, stays `starting` on its status route, fails its readiness probe and retries the broker after `KAFKA_RETRY_DELAY`.

## Quality gates and the decision records

The architecture is held in place by gates that fail: layer and context isolation, a lint policy with complexity limits, 100% line and function coverage, mutation testing, contract snapshots and a protected-configuration guard. Run them together with `make ci`; the verification section of `AGENTS.md` lists them in order, and [`docs/make-targets.md`](make-targets.md) describes each one. The decisions behind the structure are written down as records, indexed in the [decision record index](adr/README.md): twelve from the template and, from 0013 on, the decisions of the event stream and of the removal of PostgreSQL.
