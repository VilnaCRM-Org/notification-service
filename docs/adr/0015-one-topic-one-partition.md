# 0015. One topic, one partition

Planning decisions: D2 (one partition) and D3 (the event id is the offset plus 1) of the design input; FR-12, FR-15 and FR-17 of the PRD in `specs/4-realtime-event-delivery-hub/prd.md`.

## Context

A client resumes a stream on any node by sending the id of the last event it received. That id must mean the same on every node, must order every event the client can subscribe to, and must fit in an SSE `id` field and the `Last-Event-ID` header. Kafka orders records only within a partition.

## Decision

All events go to one topic, `KAFKA_TOPIC`, with exactly one partition. The id of the event at offset `o` is `o + 1`, a decimal unsigned 64-bit integer, and it is the cursor of a resume: a cursor `c` means "every id up to `c` was seen", and a resume delivers ids above `c` only. A node checks the partition count before it reads and stays `starting` (`topic-not-found`, `partition-count`) until the topic exists with exactly one partition. The service never creates the topic.

## Consequences

One total order and a single number as the cursor: a client keeps one value, any node understands it, and a subscription to several types needs no list of positions. Ids are not dense, because transaction markers, aborted and invalid records and events of other types take offsets; a gap is never a loss.

The event rate is bounded by what one partition can take and one node can read. A partition count that grows while a node runs is not noticed until the node restarts, so operators never add partitions. Cursors belong to one topic: a topic that is deleted and recreated restarts its offsets, the nodes must be restarted, and a client's cursor above the new end is clamped to it.

Growth path, not designed: several partitions with a short list of offsets, one per partition, as the cursor.

Alternatives rejected: one topic per event type (a subscription to several types would need one cursor per topic), several partitions now (a compound cursor and an order per partition only), and a cursor of its own minted by the service (it would need a single writer, [ADR 0014](0014-no-election-lock-or-link-between-nodes.md)).
