# 0005. Persistence and migrations

Planning decisions: AD-08 (persistence and transactions), AD-20 (migration policy).

## Context

The template needs a SQL adapter that can be checked at compile time, a service that starts while the database is down, and a migration approach that does not repeat the PHP defect of migrating on start. The PHP template has no entity, so a unit of work would have no user.

## Decision

`sqlx` with compile-time checked queries and committed offline metadata in `.sqlx/`, so a build needs no database. One lazily created pool, with a maximum connection count, an acquire timeout and a statement timeout taken from the configuration. Repository adapters own SQL, row structs and error translation; Domain types carry no sqlx derive. Each repository method is one statement and therefore atomic, and one command changes one aggregate. Events are published after the save succeeds and are not part of a database transaction.

Migrations are plain SQL in `migrations/`, as reversible pairs `<UTC timestamp>_<snake_case_name>.up.sql` and `.down.sql`, embedded in the binary. The service never migrates on `serve`. `service migrate` applies pending migrations after a bounded wait for the database, and `service migrate revert` reverts the most recent one. An applied migration is never edited. Each migration belongs to one bounded context and touches only that context's tables.

## Consequences

A publication failure after a successful save does not fail the command: it is logged per failing subscriber and the client receives 201, so domain-event delivery is at most once. A service that needs guaranteed delivery adds an outbox. The lazy pool lets `serve` start and answer `/ping` while the database is down. Embedding migrations means the production image needs neither `sqlx-cli` nor the directory; a build script makes cargo re-embed a new file. Changing a query requires regenerating the offline metadata.

Alternatives rejected: SeaORM and Diesel, a transactional port before any use case needs one, migrating on start, a second migration tool, one migration directory per context and a `migrate status` command.
