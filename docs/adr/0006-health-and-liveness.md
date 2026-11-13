# 0006. Health and liveness

Planning decisions: AD-11 (health check orchestration), AD-12 (liveness).

## Context

Platforms need two different answers: whether the process should be restarted, and whether it can serve traffic. Mixing them restarts healthy processes during a dependency outage. The wire contract of the PHP template is `GET /api/health` with 204 or 500 and `GET /ping` with `pong`.

## Decision

Liveness is `GET /ping`: status 200, `text/plain`, body `pong`. It is registered outside `/api` and outside the OpenAPI router and touches no dependency. The container healthcheck runs `service healthcheck`, which requests `/ping`, so a dependency outage never restarts a healthy process.

Readiness is `GET /api/health`. The port is `HealthProbe` and `Dependency` has three variants with wire identifiers `database`, `cache` and `message_broker`. `HealthService::check` runs all probes concurrently, each under `HEALTH_PROBE_TIMEOUT`, and collects every failure in registration order. With no failure the answer is 204 with an empty body; otherwise it is a 500 problem of type `/problems/dependency-unavailable` whose detail joins the fixed failure messages with a semicolon and a space and whose `failures` lists each dependency. The cause is logged at WARN and never returned. The database probe runs `SELECT 1`, the cache probe sets and reads a value, and the broker probe calls the read-only `ListQueues` operation. The handler calls the service directly, not through the query bus, so a health check does not depend on bus registration.

## Consequences

Total latency is that of the slowest probe. Configuration validation requires the outer probe timeout to be at least the adapters' own limits, so a refused connection is reported with its real cause. Adding a probe means one adapter and one line in `Adapters::connect`. Readiness does not change during shutdown, because no draining flag ships. `/ping` passes through the same middleware as every route, so it carries the request id and the security headers and appears in the access log.

Alternatives rejected: liveness under `/api/health/live`, sequential probes, reporting only the first failure, a 503 for readiness and a creating probe for the broker.
