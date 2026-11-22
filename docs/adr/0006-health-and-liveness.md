# 0006. Health and liveness

Planning decisions: AD-11 (health check orchestration), AD-12 (liveness).

## Context

Platforms need two different answers: whether the process should be restarted, and whether it can serve traffic. Mixing them restarts healthy processes during a dependency outage. The wire contract of the PHP template is `GET /api/health` with 204 or 500 and `GET /ping` with `pong`.

## Decision

Liveness is `GET /ping`: status 200, `text/plain`, body `pong`. It is registered outside `/api` and outside the OpenAPI router and touches no dependency. The container healthcheck runs `service healthcheck`, which requests `/ping`, so a dependency outage never restarts a healthy process.

Readiness is `GET /api/health`. The port is `HealthProbe` and `Dependency` has two variants with wire identifiers `cache` and `event_stream` (the event stream since [ADR 0019](0019-readiness-follows-the-freshness-of-the-tail.md); the template's database identifier and its probe went with PostgreSQL, [ADR 0020](0020-postgresql-is-removed.md), and the identifier `message_broker` and its SQS probe went with SQS, [ADR 0023](0023-sqs-is-removed.md)). `HealthService::check` runs all probes concurrently, each under `HEALTH_PROBE_TIMEOUT`, and collects every failure in registration order. With no failure the answer is 204 with an empty body; otherwise it is a 500 problem of type `/problems/dependency-unavailable` whose detail joins the fixed failure messages with a semicolon and a space and whose `failures` lists each dependency. The cause is logged at WARN and never returned. The cache probe sets and reads a value in memory, and the event-stream probe reads the age of the tail that the node keeps, so no probe calls a remote operation ([ADR 0023](0023-sqs-is-removed.md)). The handler calls the service directly, not through the query bus, so a health check does not depend on bus registration.

## Consequences

Total latency is that of the slowest probe. No probe opens a connection, so the outer probe timeout bounds local work only; the rule that tied it to the SQS operation time-out went with [ADR 0023](0023-sqs-is-removed.md). Adding a probe means one adapter and one line in `Adapters::connect`. Readiness does not change during shutdown, because no draining flag ships. `/ping` passes through the same middleware as every route, so it carries the request id and the security headers and appears in the access log.

Alternatives rejected: liveness under `/api/health/live`, sequential probes, reporting only the first failure, a 503 for readiness and a creating probe for the broker.
