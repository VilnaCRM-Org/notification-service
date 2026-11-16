# 0013. Every node reads Kafka directly

Planning decisions: D1 (every node receives every event and filters it itself) of the design input; FR-17, FR-22 and NFR-12 of the PRD in `specs/4-realtime-event-delivery-hub/prd.md`.

## Context

The service delivers the events that other services publish to frontends over SSE and WebSocket, on several nodes behind a load balancer. A client must be able to reconnect to any node and resume where it stopped, and the loss or start of a node must not disturb the others. The first plan stored events in a PostgreSQL log fed from SQS and needed a leader to write it. The owner decided on 2026-11-14: Kafka, and Rust nodes that all receive all events and filter them themselves.

## Decision

Producers write events to one Kafka topic. Each node tails that topic itself with a consumer that joins no consumer group: it is assigned the one partition, commits no offset and keeps its position in memory only. It reads with read-committed isolation and without the client's automatic offset reset; it compares offsets with the cursors itself. Each node checks every record, keeps the valid events in its memory window and hands each event to its own subscribers of the event's type. The tail is one supervised background loop per node (`TailEvents`); every broker call in it has its own time-out, and a failed round is retried from the same position after `KAFKA_RETRY_DELAY`.

## Consequences

There is no coordination between nodes: any node serves any client, nodes are stateless apart from their connections, their position and their window, and adding a node needs no configuration on the others. A connected client causes no broker read per event: the tail reads the topic once per node for all its clients.

The broker serves the whole topic once per node, and every node parses and validates every event, including the events of types nobody on that node subscribed to. Adding nodes adds room for subscribers, not event rate: the rate the system can carry is bounded by one partition and by what one node can read and validate. A node that starts reads `EVENT_WINDOW_START_EVENTS` offsets below the end of the topic before it accepts subscriptions.

Alternatives rejected: a consumer group that spreads partitions over the nodes (each node would see only part of the events and would have to forward the rest to the nodes that hold the subscribers), one reading node that forwards events to the others (a leader and links between nodes), and the PostgreSQL event log fed from SQS of the first plan.
