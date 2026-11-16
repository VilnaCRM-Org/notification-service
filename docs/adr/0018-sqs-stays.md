# 0018. SQS stays

Planning decisions: D11 (SQS and LocalStack stay) of the design input; FR-29 of the PRD in `specs/4-realtime-event-delivery-hub/prd.md`.

## Context

The template ships an SQS client factory, a readiness probe of the message broker that calls `ListQueues`, the `AWS_*` and `SQS_*` variables and a LocalStack container in the development stack. The first plan used an SQS queue as the source of its events. In this design the events come from Kafka ([ADR 0013](0013-every-node-reads-kafka-directly.md)), so the notification code has no use for SQS.

## Decision

The template's SQS parts and LocalStack stay as the template ships them. The notification code does not use them. `AWS_REGION` stays required, and the readiness probe `message_broker` stays registered next to the probe of the event stream. The queue and relay variables of the first plan's SQS source do not exist.

## Consequences

The service keeps the template's shape, so template sync stays simple and the SQS probe keeps its tests and documentation. A deployment still needs an SQS endpoint and credentials that allow `sqs:ListQueues`, or `GET /api/health` reports the message broker as failing, although no event depends on SQS. The development stack keeps starting LocalStack.

Removing the SQS parts, the probe and LocalStack is a follow-up the owner may ask for; it would touch the template's files and the protected tests of the stack.

Alternatives rejected: removing SQS and LocalStack in this change (outside the scope the owner set), and keeping SQS as a second event source next to Kafka (two orders of events and two kinds of cursor).
