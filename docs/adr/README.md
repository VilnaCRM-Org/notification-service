# Architecture decision records

These records explain why the structure of the service is what it is. Each one has the same parts: the context that forced a decision, the decision, and its consequences. Each names the planning decisions it summarises, so a reader can trace a record back to its planning bundle. Records 0001 to 0012 come from the template (`AD-nn` are its core architecture decisions, `DD-nn` its delivery decisions); the records from 0013 on are the decisions of the event stream and of the removal of PostgreSQL and name the design decisions (`Dn`) and the requirements (`FR-n`, `NFR-n`) of the bundle `specs/4-realtime-event-delivery-hub/`. The [architecture overview](../architecture.md) shows how the decisions fit together.

| Record | Title | Planning decisions |
| --- | --- | --- |
| [0001](0001-workspace-and-layers.md) | Workspace and layers | AD-01, AD-02 |
| [0002](0002-architecture-gate.md) | Architecture gate | AD-03, AD-25 |
| [0003](0003-error-model.md) | Error model | AD-06 |
| [0004](0004-configuration.md) | Configuration | AD-07 |
| [0005](0005-persistence-and-migrations.md) | Persistence and migrations (superseded by [0020](0020-postgresql-is-removed.md)) | AD-08, AD-20 |
| [0006](0006-health-and-liveness.md) | Health and liveness | AD-11, AD-12 |
| [0007](0007-api-contracts.md) | API contracts | AD-16, AD-17, AD-18, AD-19, DD-10 |
| [0008](0008-tests-coverage-mutation.md) | Tests, coverage and mutation | AD-23, AD-24, DD-09 |
| [0009](0009-make-execution-model.md) | Make execution model | DD-01, DD-02 |
| [0010](0010-workflow-layout.md) | Workflow layout | DD-08, DD-13 |
| [0011](0011-release-flow.md) | Release flow | DD-11, DD-12 |
| [0012](0012-container-images.md) | Container images | DD-19, DD-20 |
| [0013](0013-every-node-reads-kafka-directly.md) | Every node reads Kafka directly | D1, FR-17, FR-22, NFR-12 |
| [0014](0014-no-election-lock-or-link-between-nodes.md) | No election, no lock, no link between nodes | D1, FR-22, NFR-7 |
| [0015](0015-one-topic-one-partition.md) | One topic, one partition | D2, D3, FR-12, FR-15, FR-17 |
| [0016](0016-krafka-pinned-behind-a-port.md) | krafka pinned behind a port, rdkafka as the alternative | D9, FR-30, NFR-19 |
| [0017](0017-at-least-once-with-client-side-deduplication.md) | At least once, client-side deduplication | D4, D8, FR-18, FR-23, NFR-8 |
| [0018](0018-sqs-stays.md) | SQS stays | D11, FR-29 |
| [0019](0019-readiness-follows-the-freshness-of-the-tail.md) | Readiness follows the freshness of the tail | design brief 3.7, FR-20, NFR-5, NFR-6 |
| [0020](0020-postgresql-is-removed.md) | PostgreSQL is removed | D10, FR-31, FR-32, FR-33, FR-34 |

A service keeps its own records in this directory next to the twelve of the template, as this one does from 0013 on. To add one, copy the form of an existing record, number it with the next free four-digit prefix and add a row to the table above. A superseded record stays as history: its title and first line name the record that replaces it.
