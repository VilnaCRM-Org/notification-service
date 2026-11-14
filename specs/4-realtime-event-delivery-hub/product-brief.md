---
stepsCompleted: [autonomous-create-brief]
inputDocuments:
  - https://github.com/VilnaCRM-Org/notification-service/issues/4
date: 2026-11-14
author: bmad-autonomous-planning (create-brief stage, Kafka-direct revision)
---

# Product Brief: Real-time event delivery hub

Issue #4 (2026-11-14); context `notification/event-stream`. "AC" is the issue's criterion number. Replaces the first plan.

## Problem

VilnaCRM services publish events to Kafka, which a browser cannot read, so web clients poll. No shared component lists event types, pushes events to browsers, resumes a lost connection, survives a node loss and isolates slow clients.

## Users

| User | Need |
| --- | --- |
| Frontend developers | A catalog, SSE and WebSocket subscriptions, resume, clear errors |
| Publishing teams | One topic, one JSON envelope (`id`, `type`, `occurredAt`, `data`) |
| Operators | Stateless nodes, a status route, bounded memory, clean shutdown |

## Goals and scope

1. Catalog from `EVENT_TYPES`: `GET /api/event-types` and GraphQL `eventTypes`.
2. Subscriptions by type: `GET /api/events/stream` (SSE) and `GET /api/events/ws` (WebSocket, run-time commands).
3. Every node reads the one-partition topic itself, without a consumer group; no leader, lease, lock or link between nodes.
4. Ordered, resumable delivery on any node: the cursor is the offset plus 1; a memory window, then bounded replay from Kafka. At least once; clients deduplicate by the envelope `id`.
5. Isolation of slow clients: bounded queues, catch-up on the connection, a subscriber cap, 503 answers; CORS allow-list; supervised reader; graceful shutdown.
6. Node status: `GET /api/events/status` and GraphQL `eventStreamStatus`.
7. `krafka =0.32.0` behind a domain port, read-only.
8. One Kafka broker in the stack and CI; a K6 smoke scenario.
9. PostgreSQL and the example context `company-subdomain/some-module` removed, as the last epic.
10. The quality bar holds without exception.

## Success criteria

| # | Measurable criterion | AC |
| --- | --- | --- |
| S1 | REST and GraphQL return the same catalog from `EVENT_TYPES`, page by page; both are in the committed snapshots | 1 |
| S2 | An event written to the topic reaches an SSE subscriber of its type with `id`, `event`, `data`; an event of another type does not | 2 |
| S3 | A WebSocket client subscribes and unsubscribes at run time and gets JSON text frames | 3 |
| S4 | A client reconnecting to another node with its last id gets every retained later event of its types, in order, before live events | 4 |
| S5 | An event written twice by a producer arrives twice with the same envelope `id` | 5 |
| S6 | With two nodes on one topic, an event reaches subscribers of both; no consumer group or coordination exists | 6 |
| S7 | 503 while starting (until the node reaches the end read at start), shutting down or full; the status route reports state, position, subscribers | 7 |
| S8 | No `sqlx`, `DATABASE_URL`, migrations, database container or example context remains; `make ci` passes | 8 |
| S9 | No domain or application crate names a Kafka client | 9 |
| S10 | An invalid record is skipped, logged and counted in `rejectedRecords`; later records are delivered | 10 |
| S11 | A full queue delays nobody else and its subscriber catches up on its connection; a stalled WebSocket is closed after `STREAM_KEEP_ALIVE` | 11 |
| S12 | A bad type name, more than 100 types and a malformed cursor each give an RFC 9457 problem | 12 |
| S13 | A listed origin can use the catalog and both streaming routes; unset, no other origin can | 13 |
| S14 | A node whose reader loop died exits with status 1 after the normal shutdown | 14 |
| S15 | Integration and end-to-end tests run against the one broker of the stack, locally and in CI | 15 |
| S16 | The K6 smoke scenario holds 50 SSE and 50 WebSocket subscribers while events are published and stays within its budgets | 16 |
| S17 | Every local gate of `make ci` passes; coverage 100 %; no surviving mutant | 17 |

Design target, not measured and not a gate: 10 000 subscribers per node (4 vCPU, 8 GiB), p99 fan-out below 100 ms at 100 events/s.

## Out of scope

Subscriber authorization; subscriptions narrower than a type; several partitions or topics; the schema repository; duplicate removal in the service; removing SQS and LocalStack; a routing tier; TLS client certificates.

## Needs the maintainer's approval

- `architecture.toml`: `krafka` and `tokio` in the infrastructure allow-list, `serde_json` in the application allow-list, `tokio-tungstenite` in the application dev allow-list (exact list in the architecture). Label `approved-config-change` on pull request #5.
- The PostgreSQL removal changes `protected-config.toml`, `make/gates.mk`, protected scripts and Bats tests (exact list in the architecture). It needs `approved-config-change`, a session with `ALLOW_PROTECTED_CONFIG_EDIT=1`, a preparatory pull request that drops `.sqlx` and `migrations` from the required directories, `approved-breaking-change` for the removed example routes, and the removal of the required check "Persistence query metadata".

The feature does not wait for the removal approvals. Its own allow-list approval is needed before the envelope parser and every story that uses it (the tail, the subscriptions, both streaming routes, the wiring); the scaffold, the catalog, the configuration, the broker in the stack, the domain, the window, the hub and the status route come before it.

## Assumptions

> Assumption: the client receives the whole envelope as one JSON object in `data`, because it must see the envelope `id` to deduplicate.

> Assumption: the status route `GET /api/events/status` is mirrored in GraphQL as `eventStreamStatus`, which keeps the GraphQL snapshot valid for the protected contract test after the example is removed.

> Assumption: S16's budgets are those of `tests/Load/config.json.dist`; this brief sets no number.
