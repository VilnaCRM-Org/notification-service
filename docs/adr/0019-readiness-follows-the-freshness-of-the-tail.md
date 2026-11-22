# 0019. Readiness follows the freshness of the tail

Planning decisions: section 3.7 of the design brief (readiness); FR-20, NFR-5 and NFR-6 of the PRD in `specs/4-realtime-event-delivery-hub/prd.md`. Extends [ADR 0006](0006-health-and-liveness.md).

## Context

A load balancer sends new clients to the nodes whose readiness probe passes. A node that has lost Kafka, or whose tail is stuck behind a batch it cannot decode, still holds its clients and can serve resumes from its memory window, but it delivers no new event. A probe that reads the broker on every health request would load the broker and tie the probe's answer to the broker's latency.

## Decision

A node accepts subscriptions from the first time its position reached the end of the topic (`started`); until then it answers the 503 `the node is starting`. The readiness probe `event_stream` (failure message `Event stream is not ready`) is added to `GET /api/health`. It reads no broker: it reads the state the tail publishes after every round, and it passes while the node is started and its last successful watermark read and the last time its position reached the end of the topic are each at most `EVENT_TAIL_STALE_AFTER` old (default 10 seconds). Otherwise it fails with the first reason that holds, which is logged as the cause: the start-up fault of the topic, `the node is starting`, `the topic is unreachable`, `the event tail is behind`. The probe lives in the composition root, because the contexts `event-stream` and `health-check` may not depend on each other. A timestamp later than the clock's present (a clock stepped back) counts as fresh.

## Consequences

A node cut off from Kafka, or whose tail is stuck, stops receiving new clients from a load balancer that follows the probe, while the other nodes deliver; it recovers by itself when the tail reads again. A started node whose probe fails still accepts subscriptions, so a client that reaches it is served from its window, and an outage of the broker does not make every node refuse every client at once. A broker outage that hits every node fails every probe after `EVENT_TAIL_STALE_AFTER`, which the platform sees as an outage of the service, as it is for new events.

The health answer keeps fixed messages and lists the event stream after the cache and message-broker probes. [ADR 0023](0023-sqs-is-removed.md) (2026-11-22) later removed the message-broker probe with SQS. The status route `GET /api/events/status` shows the state behind the probe (`state`, `reason`, `brokerReachable`, `lag`). As in ADR 0006, no draining flag changes readiness during the shutdown sequence; the closed listener and the 503 `the node is shutting down` turn new clients away.

Alternatives rejected: readiness equal to `started` (a node cut off from Kafka would keep receiving clients), a probe that reads the broker (load and latency of the broker in every probe), and refusing subscriptions while the probe fails (clients a node could serve from its window would be turned away, and nodes would flap with the broker).
