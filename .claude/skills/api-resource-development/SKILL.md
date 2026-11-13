---
name: api-resource-development
description: Use when adding or changing a REST or GraphQL operation, a command, a query, an event subscriber or a migration of a bounded context.
---

# API Resource Development

## Use This Skill When

- A REST endpoint, GraphQL field, command, query, event subscriber or migration is added or changed.
- A new bounded context is needed for a new resource.

## Commands

- `make new-context group=<group> context=<context>` creates the three crates of a context.
- `make create-migration name=<snake_case_description>` creates a reversible migration pair.
- `make migrate` applies pending migrations to the development database.
- `make sqlx-prepare` refreshes `.sqlx/` after a query or migration changes.
- `make generate-openapi-spec` and `make generate-graphql-spec` refresh the contract snapshots.
- `make unit-tests` and `make e2e-tests` prove the behaviour.

## Rules

- Copy the shape of the example slice under `crates/company-subdomain/some-module/`:
  - domain, in `domain/src/`: `aggregate.rs`, `event.rs`, `error.rs`, `repository.rs`, `value_object/`;
  - application, in `application/src/`: `command/`, `query/`, `subscriber/`, `http/dto.rs`, `http/handler.rs`, `graphql.rs`, `error.rs`;
  - infrastructure, in `infrastructure/src/`: `persistence.rs`, with its integration tests under `infrastructure/tests/integration/`.
- Follow the recipes of the section "Recipes" below: add a REST endpoint, a GraphQL field, a command, a query, an event subscriber, a migration, a bounded context, a health probe.
- Extract request data with `ApiPath`, `ValidatedQuery`, `ValidatedJson` or `ApiJson` only; bare `Path`, `Query` and `Json` answer with plain text errors.
- Commands return no data: generate the id before dispatch and read the result through a query.
- A handler gets a unit test with `FakeCommandBus` or `FakeQueryBus`, and a scenario goes into a file under `features/`.
- Register new handlers in `service::bootstrap::router` and new subscribers in `service::bootstrap::event_bus`.
- Never edit a migration that has reached `main`; add a new one.
- When `make check-requirements` fails (gate G6): it runs `service check-config`, which validates every variable of `serve` (from the environment, then `.env`) without opening a socket or contacting a dependency, and prints one line `<VARIABLE>: <reason>` per problem with status 2. Usual causes: a variable that `crates/service/src/config.rs` requires has no value in `.env`, or a value has the wrong form (a URL scheme, a duration without a unit, a number outside its range). Fix the value in `.env` or the rule in `crates/service/src/config.rs`, keep its row in `docs/environment.md` in step, and re-run `make check-requirements`.
- Regenerate both contract snapshots in the same change as the code.

## Recipes

These recipes live in this skill, which the template sync delivers, so they stay available in a service that removed the planning bundle under `specs/`.

**Add a REST endpoint.**

1. Add or reuse a command or query in the context's application crate.
2. Add the DTO to `http/dto.rs` with `ToSchema`, serde and validator attributes.
3. Add the handler to `http/handler.rs` with `#[utoipa::path]`: `operation_id`, `tag`, `summary`, `description`, a 2xx response, the specific problem responses and `default`. Extract with `ApiPath`, `ValidatedQuery`, `ValidatedJson` or `ApiJson` only; bare `Path`, `Query` and `Json` answer with plain text errors.
4. Register it in the context's `router()` with `routes!`.
5. Unit-test the handler with `FakeCommandBus` or `FakeQueryBus` and add a scenario to a file under `features/`.
6. Regenerate `.github/openapi-spec/spec.yaml` with `make generate-openapi-spec` in the same change.

**Add a GraphQL field.**

1. Add the method to the context's `Query` or `Mutation` type in `graphql.rs`. It calls the same command or query as REST and converts errors with `problem_to_error`.
2. A new context adds its root structs to the tuple of `Query` and `Mutation` in `crates/service/src/graphql/schema.rs` and its state with `.data(..)` in `crates/service/src/bootstrap.rs`.
3. Regenerate `.github/graphql-spec/spec` with `make generate-graphql-spec` in the same change.

**Add a command.**

1. Create `command/<name>.rs`: the command struct, `impl Command` with `Error` set to the context's application error and `NAME` set to the type name, and the handler struct with its ports as `Arc<dyn Port>`, `new` and an inherent `async fn` with the logic. The `CommandHandler` impl only delegates.
2. The handler loads or creates one aggregate, saves it, then publishes `pull_domain_events()`. A publication failure is logged per failing subscriber and never fails the command.
3. Register it with `.register(..)?` in the private `register_commands` of `crates/service/src/bootstrap.rs` (a query: `register_queries`), which `bootstrap::router` calls.
4. Extend the wiring test in `crates/service/src/bootstrap/tests.rs`.

**Add a query.** Same as a command, in `query/<name>.rs`, with `Output` set to a view struct or a collection of views. A query handler never mutates state and returns views, not aggregates.

**Add an event subscriber.**

1. Create `subscriber/<purpose>_<event>.rs` implementing `DomainEventSubscriber<Event>`.
2. Register it with `.subscribe(..)` in `bootstrap::event_bus` of `crates/service/src/bootstrap.rs`.
3. A subscriber is idempotent and does not call the command bus.
4. Delivery is at most once: work that must not be lost needs an outbox, which the template does not ship.

**Add a migration.**

1. Run `make create-migration name=<snake_case_description>` and write the up and the down file.
2. Apply it with `make migrate`, update the adapter, run `make sqlx-prepare` and commit `.sqlx/` with the migration.
3. Never edit a migration that has reached `main`.

**Add a bounded context.** Run `make new-context group=<group> context=<context>`, then follow the steps that `docs/new-service.md` lists under "Add a bounded context": the `[workspace.dependencies]` entries, the code layer by layer with its fakes in `test-support`, the wiring in the `service` crate (adapters in `Adapters`, handlers in `bootstrap::router`, a state field in `AppState`, its router in `openapi::api_router`, its GraphQL roots in `graphql::schema`), the components in `workspace.dsl`, the migration and the regenerated snapshots. `architecture.toml` and every other protected item stay untouched.

**Add a health probe.**

1. Add a variant to `Dependency` in `crates/internal/health-check/domain/src/value_object/dependency.rs` with its identifier and failure message.
2. Implement `HealthProbe` in `crates/internal/health-check/infrastructure/src/probe.rs`. A client crate the probe needs goes into `crates/internal/health-check/infrastructure/Cargo.toml`; that manifest is synchronised, so list it by its full path in `.templatesyncignore` in the same pull request.
3. Append it to `Adapters::probes` in `Adapters::connect` of `crates/service/src/bootstrap.rs`. The handler and `HealthService` do not change.

## Verification

```bash
make unit-tests
make e2e-tests
```

Continue with `api-contract-development` for the snapshots, `testing-workflow` for failing tests and `architecture-model-sync` when a crate was added.
