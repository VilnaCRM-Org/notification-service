# 0014. No election, no lock, no link between nodes

Planning decisions: D1 (no leader) of the design input; FR-22 and NFR-7 of the PRD in `specs/4-realtime-event-delivery-hub/prd.md`.

## Context

The first plan needed one node to write the event log, so it elected a leader with a lease in PostgreSQL, renewed it on a timer and had the other nodes follow it. Election, lease and lock mechanisms were measured for that plan; the measurements are the reason they were dropped, not figures of this design. With every node reading Kafka directly ([ADR 0013](0013-every-node-reads-kafka-directly.md)) the question is whether anything still needs a single writer or a shared view.

## Decision

Nothing does. The service writes nothing to Kafka, and the cursor of an event, its Kafka offset plus 1, is the same on every node, so no node has to hand out ids or positions. The service has no leader, no election, no lease, no lock, no link or message between nodes and no shared store besides the topic. It writes no Kafka transaction and commits no consumer offset.

## Consequences

The loss or start of a node changes nothing on the others, and nodes need no configuration about each other. A client resumes on any node with the last id it received. A node that cannot serve leaves service by itself: a dead tail loop ends the process, and a node cut off from Kafka fails its readiness probe ([ADR 0019](0019-readiness-follows-the-freshness-of-the-tail.md)). The cluster context of the first plan, its node and lease variables and its leader route do not exist.

Each node holds its own position, window and counters, so two nodes can report different positions on their status routes for a moment; a client that switches nodes may receive an event again, which the client removes by the envelope `id` ([ADR 0017](0017-at-least-once-with-client-side-deduplication.md)).

Alternatives rejected: the leader lease in PostgreSQL of the first plan, an election through a consensus protocol, a distributed lock in Redis, and a Kafka transaction written by the service to coordinate nodes.
