# 0017. At least once, client-side deduplication

Planning decisions: D4 (the record value is a JSON envelope) and D8 (the client deduplicates) of the design input; FR-18, FR-23 and NFR-8 of the PRD in `specs/4-realtime-event-delivery-hub/prd.md`.

## Context

Producers can write an event twice, for example when they retry a write whose acknowledgement was lost. A client that reconnects may resume from an older id than the last one it processed. Removing duplicates in the service would need a store shared by the nodes, or state per node that differs between nodes and breaks a resume on another node ([ADR 0014](0014-no-election-lock-or-link-between-nodes.md)).

## Decision

Delivery is at least once, in offset order, for the types of a subscription, across reconnects to any node, for retained events. The service removes no duplicates: an event written twice reaches a client twice, with the same envelope `id` and two event ids. Within one connection the service never writes the same event id twice and never an id at or below the cursor of the resume; it compares offsets with the cursor itself. Every event reaches the client as the whole envelope object (`id`, `type`, `occurredAt`, `data`), so the client has the envelope `id` and deduplicates by it.

## Consequences

The nodes need no shared state and no deduplication memory, and a client can resume on any node. Every client must keep the envelope ids it handled recently and drop a repeated one; `docs/streaming-protocol.md` tells client authors how. The ids of one connection strictly increase, also after the connection fell behind and caught up. Records of aborted transactions are never delivered, because the service reads with read-committed isolation.

Events below the replay floor (more than `EVENT_REPLAY_MAX_EVENTS` offsets below the serving node's position, or no longer retained by the topic) are not delivered to a resuming client, and the client is not told. A gap signal is a follow-up; a client that suspects a loss reloads its state through the service that owns the data.

Alternatives rejected: exactly-once delivery with deduplication in the service (a shared store or per-node state), and forwarding only the producer's `data` without the envelope (the client would have nothing to deduplicate by).
