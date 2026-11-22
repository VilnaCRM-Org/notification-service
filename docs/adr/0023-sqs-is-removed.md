# 0023. SQS is removed

Planning decisions: the owner decision of 2026-11-22 to run on Kafka only and to fix what the load tests found; it reverses D11 (SQS and LocalStack stay) of the design input and changes FR-29 of the PRD in `specs/4-realtime-event-delivery-hub/prd.md`. Supersedes [ADR 0018](0018-sqs-stays.md) and amends [ADR 0006](0006-health-and-liveness.md), [ADR 0019](0019-readiness-follows-the-freshness-of-the-tail.md) and [ADR 0020](0020-postgresql-is-removed.md).

## Context

[ADR 0018](0018-sqs-stays.md) kept the template's SQS parts although no event of the service passes through SQS: the events come from Kafka ([ADR 0013](0013-every-node-reads-kafka-directly.md)). The parts were an SQS client factory in the shared infrastructure crate, the readiness probe `message_broker`, which called `ListQueues` on every request of `GET /api/health`, the `AWS_*` and `SQS_*` variables with `AWS_REGION` required, the AWS SDK crates, and a LocalStack container with its volume and port in every compose stack.

The load tests showed the cost. Each health request made one call to the emulator, so the health scenario measured LocalStack and not the service, and its throughput stopped at a few hundred requests per second. Every stack, from the development stack to the load-test stack on the EC2 instance, started one more container and waited for its health check. A production deployment needed an SQS endpoint and credentials that allow `sqs:ListQueues` only so that readiness would not fail.

## Decision

The service runs on Kafka only. SQS and LocalStack leave the service, the stacks, the tests and the documentation:

- the SQS client factory, the probe `SqsProbe` and the dependency identifier `message_broker`; the readiness report lists the cache and the event stream, and neither probe does network I/O: the cache probe works in memory, and the event-stream probe reads the age of the tail that the node already keeps ([ADR 0019](0019-readiness-follows-the-freshness-of-the-tail.md)), so the reachability of Kafka is still part of readiness;
- the variables `AWS_REGION`, `AWS_ENDPOINT_URL_SQS`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `SQS_CONNECT_TIMEOUT`, `SQS_OPERATION_TIMEOUT`, `SQS_MAX_ATTEMPTS`, `LOCALSTACK_HOST` and `LOCALSTACK_PORT`, and the rules that tied `HEALTH_PROBE_TIMEOUT` to the SQS operation time-out and the two AWS keys to each other; `KAFKA_BOOTSTRAP_SERVERS` becomes the one required variable of `serve` and `check-config`, and the production reference `docker-compose.prod.yml` requires it from its environment;
- the crates `aws-config`, `aws-sdk-sqs` and `aws-smithy-mocks`, their entries in `architecture.toml`, and the advisory exception of `deny.toml` that existed only for the test dependency of the SQS mocks;
- the LocalStack service, its volume and port 4566 in the four compose files, `DEPS=localstack` (the default of `start-deps` is `kafka`), the forwarded port of the dev container and the Dependabot rule of the LocalStack image.

The dry run of the EC2 load-test runner (`make aws-load-tests LOCAL_MODE_ENV=true`) used LocalStack for EC2, S3, IAM and STS. It stays, but calls a stub of the AWS CLI, `tests/Load/stub-aws.sh`, which answers the calls of the runner and of its cleanup with canned values and needs neither an emulator nor Docker. The real runner is unchanged: it never used SQS.

## Consequences

`GET /api/health` touches no network, so its cost is that of the service and the load tests measure the service. A deployment needs no AWS endpoint, region or credentials. The stacks start one container fewer, and the targets that need services wait only for the Kafka broker.

The body of a failed readiness check can no longer name `message_broker`. The field is a string and not an enum in the OpenAPI document, so the change is not breaking for oasdiff; the snapshot and the documentation follow. The scenario "queue service is unavailable" leaves `features/health_check.feature`, which keeps the cache scenario and the healthy one, and the hash pin of that file in `make_general.bats` follows. The protected Bats tests that used LocalStack, that pin, `deny.toml` and `architecture.toml` change with the label `approved-config-change`.

A template sync that changes the SQS parts, LocalStack or their tests would bring them back. `.templatesyncignore` lists the files that held them, and such a template change does not apply to this service. A service that later needs a queue adds an adapter of its own behind a port of its context, with a decision record.

[ADR 0006](0006-health-and-liveness.md) changes in three sentences: `Dependency` has two variants, `cache` and `event_stream`; no probe calls a remote operation; and the configuration no longer ties the probe time-out to an adapter's limit. ADR 0006 carries these sentences with a pointer to this record. The sentences of [ADR 0008](0008-tests-coverage-mutation.md) and [ADR 0012](0012-container-images.md) that named LocalStack as a backing service of the tests and of the load-test stack now name the Kafka broker alone.

[ADR 0019](0019-readiness-follows-the-freshness-of-the-tail.md) and [ADR 0020](0020-postgresql-is-removed.md) are amended without being rewritten. In ADR 0019, the sentence that lists the event stream "after the cache and message-broker probes" now reads: the health answer lists the event stream after the cache probe. In ADR 0020, the sentence that lists the cache, the message broker and the event stream in the readiness report now reads: the readiness report lists the cache and the event stream.

Alternatives rejected: keeping SQS and only removing the probe (the variables, the crates and the container would stay without a user), re-sourcing the `message_broker` probe from Kafka (the event-stream probe already covers the broker without a request per health check), keeping LocalStack in a separate compose file for the dry run (an emulator image, its pin and its update rule kept for one test), and dropping the dry run (the runner would have no automated check).
