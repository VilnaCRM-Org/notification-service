# 0016. krafka pinned behind a port, rdkafka as the alternative

Planning decisions: D9 (the Kafka client) of the design input; FR-30 and NFR-19 of the PRD in `specs/4-realtime-event-delivery-hub/prd.md`.

## Context

The service reads one Kafka partition without a consumer group: metadata, watermarks, seeks and long polls, with read-committed isolation and bounded waits. The dependency policy (`deny.toml`, checked by `make check-security`) bans OpenSSL, the builder image has no C toolchain for a native Kafka library, and the tests need broker faults on demand. The established Rust client, `rdkafka`, wraps the C library librdkafka and uses OpenSSL for TLS.

## Decision

The service uses `krafka`, a Kafka client written in Rust, pinned to the exact version `=0.32.0` in `[workspace.dependencies]`, without default features and with rustls on the aws-lc-rs provider that the workspace already uses. Its test feature, with an in-memory fake broker, appears only on dev-dependency lines. The domain crate of the event stream defines three ports, `TopicProbe` (metadata and watermarks), `TopicTail` (the tail's reader) and `TopicReplay` (pages of older records); only the infrastructure crate uses `krafka` in production code, and the composition root wires it (its one mention of the client is the log directive `krafka=error`); the service integration tests and the Gherkin suite use its producer to write test records. The adapter only reads: it produces nothing, joins no group and commits nothing.

## Consequences

`krafka` passes the dependency policy without lifting the OpenSSL ban, needs no C toolchain for its Kafka code, has request and connect time-outs, and ships a fake broker that produces the faults a real broker cannot produce on demand (a silent broker, a dropped connection, a corrupt batch). Its defaults that would be wrong for the service are overridden in the adapter: read-committed instead of read-uncommitted, no automatic offset reset instead of a silent jump to the end, no metrics push, the client id of `KAFKA_CLIENT_ID`. Its long poll reports no broker outage, so the service reads the watermarks at least once per second on a connection of its own; after an outage it seeks to its own position, because the client's internal fetch back-off would otherwise delay delivery.

Its weakness: one author, a version below 1.0, and 16 releases in 14 weeks with breaking changes in minor releases. The exact pin keeps an update deliberate, the ports keep the client out of the domain and application crates, and the read-only use keeps the used surface small. Replacing it by `rdkafka` touches only the infrastructure crate, the composition root, the manifests, and the tests that write records with its producer (the service integration tests and the Gherkin suite); it would need the OpenSSL ban lifted for TLS and a C toolchain in the builder image.

Alternatives rejected: `rdkafka` now (the OpenSSL ban and the C toolchain), and a Kafka client written in the service.
