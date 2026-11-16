# 0020. PostgreSQL is removed

Planning decisions: D10 (PostgreSQL and the example context leave the service) of the design input; FR-31, FR-32, FR-33 and FR-34 of the PRD in `specs/4-realtime-event-delivery-hub/prd.md`. Supersedes [ADR 0005](0005-persistence-and-migrations.md).

## Context

The template ships PostgreSQL for one user: its example bounded context, which stores a resource through a repository and shows how a use case, an aggregate, a domain event and a SQL adapter fit together. [ADR 0005](0005-persistence-and-migrations.md) records that persistence design. The first plan of the event stream also stored the events in a PostgreSQL log. The design that was built keeps the events only in Kafka ([ADR 0013](0013-every-node-reads-kafka-directly.md)): every node reads the topic, keeps its window in memory and writes nothing ([ADR 0014](0014-no-election-lock-or-link-between-nodes.md)). No requirement of the service reads or writes a row.

The database still cost a container and a volume in every stack, a probe in the readiness report, a migration step before the development service started, twelve variables, eight Make targets, three scripts, the gate of the offline query metadata (G30) with its CI job, and the sqlx command-line tool in that job and in the test, end-to-end, mutation and Bats jobs.

## Decision

The service stores nothing and has no database. Kafka holds all shared state: the events live only in the topic, and the position, window and subscribers of a node live in its memory. PostgreSQL leaves the service, the stack, the gates and CI, and the example context goes with it:

- the three crates of the example context, its routes, its GraphQL query and mutation fields, its OpenAPI paths and schemas, its features, steps, fixtures and tests; the GraphQL schema keeps the queries `eventTypes` and `eventStreamStatus` and has no mutation root;
- `sqlx`, the connection pool and the embedded migrations of the shared infrastructure crate, the PostgreSQL probe and the dependency identifier `database`, the migrate command of the binary, the database variables and the rule that tied the probe time-out to the acquire time-out of the pool;
- the database and migration services of the compose files and the database volume and port; the Make targets migrate, migrate-rollback, create-migration, setup-test-db, setup-load-test-db, sqlx-prepare, validate-sqlx-metadata and ensure-database, the scripts behind them, the gate G30 and the CI job of the query metadata, and the sqlx tool of every workflow.

The readiness report lists the cache, the message broker (SQS, which stays by [ADR 0018](0018-sqs-stays.md)) and the event stream. The workspace keeps thirteen crates.

## Consequences

`serve` and `check-config` need no database variable, the stacks start no database, and the first compile of the development stack runs in the `app` container instead of a migration step. A node that restarts loses its window and no event, because the events stay in the topic for its retention. The buses and the in-process cache of the shared kernel stay; no handler of this service publishes a domain event, so the at-most-once delivery of domain events after a save and the outbox that ADR 0005 named no longer apply.

Removing the example routes and fields is a breaking change of the API contract, approved with the label `approved-breaking-change`; the version of the OpenAPI document went to 2.0.0. Removing the protected scripts, gate and tests needed the label `approved-config-change`. The configuration guard reads its required directories from the base branch, so the two directories of the query metadata and the migrations stay with one placeholder file each until the base branch no longer requires them; a follow-up pull request deletes them.

A template sync that changes the example context, the database stack or their tests would bring them back. `.templatesyncignore` keeps listing those paths, and such a template change does not apply to this service. A service that later needs to store data adds an adapter of its own behind a port of its context, with a decision record.

Alternatives rejected: keeping PostgreSQL for the example context only (a container, a probe and a migration step that no requirement of the service uses), keeping the example context on an in-memory repository (a pattern the service does not use, and routes a client could mistake for the API of the service), and storing the events in PostgreSQL next to Kafka (the first plan: two orders of the events and a writer to elect).
