---
stepsCompleted: [autonomous-create-brief]
inputDocuments:
  - https://github.com/VilnaCRM-Org/notification-service/issues/4
  - README.md
  - AGENTS.md
date: 2026-11-13
author: bmad-autonomous-planning (create-brief stage)
---

# Product Brief: Real-time event delivery hub

Issue: [#4](https://github.com/VilnaCRM-Org/notification-service/issues/4) "Real-time event delivery hub: catalog, SSE and WebSocket subscriptions, clustered fan-out". Bounded contexts: `notification/event-stream` and `internal/cluster`. "AC n" below means acceptance criterion n of issue #4.

## Summary

The notification service becomes the single place where VilnaCRM web clients receive domain events in real time. A client reads the catalog of subscribable event types over REST or GraphQL, opens a Server-Sent Events (SSE) or WebSocket subscription for the types it needs, and then receives each matching event as soon as it arrives from the VilnaCRM event bus. The service runs as a cluster of identical nodes on one PostgreSQL database: any node serves any client, one node at a time holds a leader lease and ingests from the bus, and every node relays the shared event log to its own subscribers.

## Problem statement

VilnaCRM backend services publish domain events to an event bus, but a browser cannot consume that bus. Today a web client learns about a change only by polling a REST or GraphQL endpoint, which adds latency for the user and load for the backend services. There is no shared component that:

- tells a client which event types exist;
- pushes events to browsers over a protocol they support natively;
- lets a client that lost its connection resume without missing or repeating events;
- keeps working when a node is lost or restarted;
- protects the other clients and the ingestion path from one slow client.

Without it each backend team would have to build its own push channel, each with its own protocol and failure behaviour.

## Users and their jobs

| User | Job to be done | What they need from this service |
| --- | --- | --- |
| Frontend developers of VilnaCRM web clients | Show a change in the UI when it happens, without polling | A catalog of event types (REST and GraphQL), an SSE endpoint usable with the browser `EventSource`, a WebSocket endpoint with run-time subscribe and unsubscribe, resume by `Last-Event-ID`, RFC 9457 problem responses for invalid input |
| Backend teams that publish domain events | Reach web clients without owning client connections | One ingestion contract on the event bus (event type, optional event id and time, opaque text body), idempotency by event id, a defined order, no change to their own service when the transport behind the port changes |
| Operators who run the cluster | Keep delivery available during node loss, deployments and load | Identical stateless nodes behind a load balancer, automatic leader failover, a status endpoint, bounded memory per client, graceful shutdown that hands clients to another node, configuration through documented environment variables |

## Goals

1. A client can discover event types and subscribe to them over SSE or WebSocket with no knowledge of the event bus.
2. Delivery is ordered and resumable: the event log sequence is the total order, the SSE `id` and the resume cursor.
3. The cluster has no single point of failure in the service tier: a node loss costs its clients one reconnect, and a leader loss pauses ingestion for at most the lease time.
4. Fan-out cost is proportional to the number of interested subscribers, not to the number of connections, and a connected client causes no per-event database read.
5. The event-bus transport is replaceable behind a port, so the Kafka adapter and the schema repository can be added later without a change to the domain or application crates.
6. The change meets the repository quality bar without any exception: every gate of `make.ci` that can run locally passes.

## Scope

### In scope

- Event-type catalog: `GET /api/event-types` (paginated) and the GraphQL query `eventTypes`, both in the committed contract snapshots.
- SSE subscription `GET /api/events/stream?types=a,b` with `id`, `event` and `data` fields and a keep-alive comment.
- WebSocket subscription `GET /api/events/ws` with JSON `subscribe` and `unsubscribe` commands and JSON text frames.
- Resume after `Last-Event-ID` (SSE header, `lastEventId` query parameter) within the retention window (`EVENT_RETENTION`, default 1h).
- A PostgreSQL event log shared by all nodes, with a relay on every node and pruning by the leader.
- Leader election by a PostgreSQL lease with a monotonically increasing term, and `GET /api/cluster`.
- The `EventSource` ingestion port and its first adapter, which consumes the SQS queue of the local stack.
- Slow-consumer protection: a bounded queue per subscriber, disconnection on overflow, and a cap on subscribers per node.
- Unit, integration, two-node, end-to-end and K6 smoke tests, the documentation of the streaming protocols under `docs/`, and the update of `workspace.dsl`.

### Out of scope

| Item | Follow-up |
| --- | --- |
| Subscriber authorization and private topics | A later issue that validates a JWT issued by the user-service OAuth server. Until then the service must carry only events that are safe for every client; the security notes state this. |
| Kafka adapter | A later issue that implements `EventSource` for Kafka. It adds a crate to the protected allow-list, so it needs a maintainer-approved change with the `approved-config-change` label. |
| Schema repository integration | A later issue, once the schema repository exists. `data` stays opaque UTF-8 text until then; the catalog holds names only. |
| Raft consensus | None planned. An ADR records why the PostgreSQL lease is the election mechanism. |
| Removal of the example context `company-subdomain/some-module` | The later guided story of the new-service procedure. |
| PostgreSQL high availability | A deployment prerequisite, owned by the operators; not built by this service. |

## Success criteria

| # | Criterion (measurable) | Traces to |
| --- | --- | --- |
| S1 | The REST endpoint and the GraphQL query return the same catalog, page by page, and both contracts are in the committed OpenAPI and GraphQL snapshots. | AC 1 |
| S2 | An event of a subscribed type that enters through the event bus reaches an SSE client with `id`, `event` and `data`; an event of another type does not. | AC 2 |
| S3 | A WebSocket client subscribes and unsubscribes at run time with JSON commands and receives matching events as JSON text frames; an invalid command gets an error frame and the connection stays open. | AC 3 |
| S4 | A client that reconnects with a cursor receives every retained event after it, in sequence order, without duplicates, before any live event. | AC 4 |
| S5 | With two nodes on one database, an event ingested by one node reaches subscribers of the other node. | AC 5 |
| S6 | At any time at most one node holds the lease; after the leader stops, another node holds it within `CLUSTER_LEASE_TTL` (default 10s) and ingestion continues; `GET /api/cluster` reports node id, role, leader and term. | AC 6 |
| S7 | The domain and application crates of `notification/event-stream` contain no reference to SQS; the SQS adapter passes its integration tests against the local stack. | AC 7 |
| S8 | A subscriber that stops reading is disconnected when its queue (`SUBSCRIBER_BUFFER`, default 256) is full, while other subscribers and ingestion continue without delay. | AC 8 |
| S9 | An invalid event type name, more than 100 types, and a malformed `Last-Event-ID` each produce an RFC 9457 problem response. | AC 9 |
| S10 | The K6 smoke scenario holds 50 SSE subscribers while events are published and stays inside its latency budget. | AC 10 |
| S11 | Every gate of `make.ci` that can run locally passes; line and function coverage stay at 100 % and no mutant survives in the changed code. | AC 11 |
| S12 | Capacity target for the PRD: 10 000 concurrent subscribers per node on the reference machine (4 vCPU, 8 GiB) with p99 fan-out latency below 100 ms at 100 events/s. | AC 8, AC 10 (design target beyond the smoke budget) |

## Positioning against Mercure

Mercure is the established open protocol and hub for pushing updates to browsers over SSE, and this service follows its main ideas: opaque text payloads, topic-style selection and resume by `Last-Event-ID`. The differences are deliberate. Clustering is part of this open-source service and uses the PostgreSQL database that the service already depends on, events enter from the VilnaCRM event bus through a port and not through an HTTP publish endpoint, and WebSocket is offered next to SSE. This service does not implement the Mercure protocol itself, so Mercure client libraries, its publish API and its JWT-based authorization are not available here; authorization is a named follow-up.

## Key risks

| Risk | Effect | Mitigation in this plan |
| --- | --- | --- |
| No subscriber authorization in this issue | Any client can read any event type | Carry only events that are safe for every client; state it in the security notes; follow-up issue for JWT |
| PostgreSQL is the single consistent store | An outage pauses new events and elections | Existing connections stay open; a node whose lease store fails steps down to follower; PostgreSQL HA is a deployment prerequisite |
| Serialised appends (advisory lock) bound the ingestion rate | Ingestion throughput has a ceiling | Batched appends; the target is 100 events/s; measure in the load tests before raising the target |
| Leader loss | Ingestion pauses for up to the lease time | Lease released on graceful shutdown, so a planned restart fails over at once; expiry is judged by the database clock, so clock skew cannot give two leaders |
| A client offline longer than the retention window | Events older than `EVENT_RETENTION` are lost for that client | Document the window; the client reloads its state through the owning service's API |
| Slow clients and connection floods | Memory growth, or delay for other clients | Bounded queue with eviction, `MAX_SUBSCRIBERS` cap answered with 503 |
| Streaming routes are outside OpenAPI | Their contract is not covered by the snapshot and Schemathesis gates | Specify them in `docs/` and cover them with end-to-end features |
| 10 000 subscribers per node is unproven in CI | The capacity target may not hold | The CI gate is the 50-subscriber smoke scenario; the capacity target is checked with the on-demand load profiles |
| Quality bar on concurrent code (100 % coverage, no surviving mutant, complexity 8) | Schedule risk for hub, relay and elector | Small functions, time only through the `Clock` port and tokio time, fakes for every port |
| Kafka adapter depends on a protected-config approval | The production bus cannot be connected by this issue alone | The port and the attribute-plus-body envelope are fixed now so the adapter is a contained follow-up |

## Assumptions

> Assumption: The SSE query parameter is the comma-separated `types`, not `?type=<name>` as the issue text writes it; one parameter then serves one or many types and matches the WebSocket `types` field.

> Assumption: Leader election is a PostgreSQL lease with a fencing term, which is the "other election mechanism" the product intent allows; Raft is rejected because no Raft crate is on the protected allow-list and it would add a second consensus system and failure domain.

> Assumption: The first ingestion adapter is SQS on the local stack, because `aws-sdk-sqs` is already allowed in infrastructure crates and a Kafka client is not; the Kafka adapter is a follow-up.

> Assumption: Event `data` is opaque UTF-8 text limited by `EVENT_MAX_DATA_BYTES` (default 65536), as in Mercure; no schema is validated until the schema repository exists.

> Assumption: This service is an alternative to the Mercure hub, not an implementation of the Mercure protocol; wire compatibility with Mercure clients is not a goal of issue #4.

> Assumption: The positioning statement relies on the public description of Mercure (SSE transport, HTTP publish endpoint, JWT authorization, clustering outside the free open-source hub); the PRD stage should not add finer claims about Mercure without a source.

> Assumption: S12 (10 000 subscribers, p99 below 100 ms) is a design target carried to the PRD as an NFR and is not a merge gate; the merge gate for performance is the K6 smoke scenario of AC 10.

> Assumption: The latency budget of the K6 smoke scenario is the budget defined for that scenario under `tests/Load/`; this brief sets no number for it.

> Assumption: Make targets are named by profile key (`make.ci`); the key is formed from the target name because the repository does not define the keys itself.

> Assumption: The three user groups are internal to VilnaCRM; external integrators are not a user group for this issue because there is no subscriber authorization yet.

> Assumption: "Ingestion continues" in AC 6 means the new leader resumes reading the queue; messages not yet acknowledged by the old leader are redelivered by the bus and deduplicated by event id.
