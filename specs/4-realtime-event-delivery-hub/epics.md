# Epics and Stories: real-time event delivery hub (issue #4)

This file slices the design of `architecture.md` of this bundle into stories for the contexts `notification/event-stream` and `internal/cluster`. It adds no component: every file, type, limit, query number (Q1 to Q12) and message text named below is the one of `architecture.md` and `prd.md`. A decision taken while slicing is a line `> Assumption:` in the story it affects.

**How the stories run:**

- A story is independent when the stories of its `Depends on:` line are done. Independent stories run in one round as parallel sessions in one working tree. A session runs only the DB-free targets `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity`. One serialized verify step per round then runs the DB-backed targets `make.setup_test_db`, `make.sqlx_prepare`, `make.integration_tests`, `make.tests_with_coverage` and `make.e2e_tests`, and each story is committed on its own through the pre-commit hook.
- Two stories of one round never write the same file. Each story lists the files it owns under "Scope". A file with several writers is chained through `Depends on:` edges, also where no code dependency exists; the story text calls such an edge "ownership".
- A dependency enters a member manifest in the story whose code first uses it, because the unused-dependency gate of the hook refuses an entry without a use. A new manifest entry also changes `Cargo.lock`, so at most one story per round changes a manifest.
- A crate-private item is added by the story of its first caller, because an item without a caller fails the lint gate.
- Every story carries its own tests to the bar of the repository: 100 % line and function coverage of its code, both sides of every boundary, every error branch and every log entry asserted, every awaiting test bounded by `tokio::time::timeout`, paused tokio time wherever a timer is involved. No story plans a lint suppression, a coverage or mutation exclusion, a protected-file edit or a lowered bar.

**Single-writer chains (ownership edges):**

- `Cargo.lock` and the manifests: 1.1, 1.2, 3.2, 3.4, 3.5, 4.3, 5.1, 6.2, 6.4.
- `crates/service/**`: 1.1, 1.2, 2.1, 4.3, 5.2, 5.3, 6.1, 6.4.
- `workspace.dsl`: 1.1, 1.2, 5.1, 6.5. Contract snapshots: 1.1, 1.2. `migrations/` and `.sqlx/`: 1.1, 1.2, 3.4.
- `tests/support/**`: 1.1, 1.2, 3.1. `tests/e2e/**` and `features/`: 1.1, 1.2, 6.2.
- `docs/environment.md` and `.env`: 1.2, 2.1, 2.2, 6.3. Compose files: 2.2, 6.3. `.templatesyncignore`: 1.1, 1.2, 2.2, 5.2, 5.3, 6.3, 6.5.
- `crates/notification/event-stream/application/src/lib.rs`: 1.1, 3.2, 3.3, 3.5; its `src/http.rs`: 1.1, 3.2, 4.1, 4.2, 4.3, 4.4.

**Expected rounds:**

- Round 1: 1.1
- Round 2: 1.2
- Round 3: 2.1, 3.1
- Round 4: 2.2, 3.2
- Round 5: 3.3, 3.4, 4.1
- Round 6: 3.5, 4.2
- Round 7: 4.3
- Round 8: 4.4, 5.1, 5.2
- Round 9: 5.3
- Round 10: 6.1, 6.2, 6.3
- Round 11: 6.4, 6.5
- Round 12: 6.6

## Epic 1: Walking skeletons of the two bounded contexts

Both contexts are absent from `[workspace].members`, so the epic scaffolds each one and proves it with the smallest vertical slice of the design: the event-type catalog for `notification/event-stream` and the leader lease with its status endpoint for `internal/cluster`. After the epic the service has six new crates, two documented operations, one GraphQL query and two of the three new tables.

### Story 1.1: Scaffold notification/event-stream

As a frontend developer, I read the catalog of subscribable event types over REST and GraphQL, served by the new context `notification/event-stream`. The story is the walking skeleton of the context: the three crates through the scaffolder and only the catalog slice of architecture.md, wired end to end. It re-checks both names against the live tree before C1, runs alone and has the test database to itself; nothing about the log, the hub, the streams or ingestion is written here.

**Scope:**

- Scaffolder (`make.new_context` with `group=notification context=event-stream`): `crates/notification/event-stream/{domain,application,infrastructure}`, the three sorted member paths and, at first use, the three path entries of `[workspace.dependencies]` in the root `Cargo.toml`; `Cargo.lock`.
- Domain: `crates/notification/event-stream/domain/src/{lib.rs,error.rs,value_object.rs,port.rs}`, `src/value_object/event_type.rs`, `src/port/event_catalog.rs`, each with its sibling tests.
- Application: `crates/notification/event-stream/application/src/{lib.rs,error.rs,query.rs,http.rs,graphql.rs}`, `src/query/list_event_types.rs`, `src/http/{dto.rs,handler.rs}` (`ListEventTypesParams`, `EventTypeResponse`, `list_event_types`, `list_types`, `EventStreamState { query_bus }`, `router()` with the one documented route).
- Infrastructure: `crates/notification/event-stream/infrastructure/src/{lib.rs,persistence.rs}`, `src/persistence/event_catalog.rs`, `tests/integration/{main.rs,migration.rs,event_catalog.rs}`.
- Database: the pair `migrations/<timestamp>_create_notification_event_types.{up,down}.sql`, two new files in `.sqlx/` (Q8, Q9).
- Test support: `tests/support/Cargo.toml`, `tests/support/src/{lib.rs,event_catalog.rs,fixture.rs}`.
- Composition root: `crates/service/Cargo.toml`, `crates/service/src/{bootstrap.rs,http.rs,openapi.rs}`, `crates/service/src/graphql/schema.rs`, the three `Adapters { .. }` literals (`src/bootstrap/tests.rs`, `src/http/router/tests.rs`, `tests/integration/panic.rs`) and the two pinning tests (`src/openapi/tests.rs`, `src/graphql/schema/tests.rs`).
- Model, contracts, end to end, documentation: `workspace.dsl`, `.github/openapi-spec/spec.yaml`, `.github/graphql-spec/spec`, `features/event_catalog.feature`, `tests/e2e/{steps.rs,world.rs}`, `tests/e2e/steps/event_catalog.rs`, `docs/requests.http`, `docs/architecture.md`, `.templatesyncignore`.

**Covers:** FR-1, FR-2, FR-3 (idempotent registration in the adapter), NFR-18, NFR-20.

> Assumption: the catalog table gets its own migration pair `create_notification_event_types` here, and the table `notification_events` gets its pair in story 3.4. architecture.md puts both tables into one pair; a walking skeleton must not create a table no code reads, and an applied migration is never edited. The SQL of each table is the one of architecture.md, so the change has three new pairs instead of two.
>
> Assumption: `InvalidEvent` starts with the variants `TypeLength` and `TypePattern`; story 3.1 adds the other four with their codes.
>
> Assumption: `API_VERSION` becomes `1.1.0` in this story, the first that adds an operation; story 1.2 adds its operation inside the same unreleased minor version.
>
> Assumption: the application manifest declares `tokio` only as a dev-dependency here; the stories that first use `tokio`, `tracing`, `humantime` and `futures-util` in production code add them.
>
> Assumption: in C12 a target that needs the production image is recorded `not run` with its reason and left to CI when the image cannot be built in the session; the other stages still run one call each.

**Acceptance Criteria:**

- **Given** the names `notification` and `event-stream` pass the name rule, the reserved-name rule and the existence rule against the live tree (re-checked by the story before any write), **When** the target of `make.new_context` runs with `group=notification context=event-stream`, **Then** C1 is done: three crates exist under `crates/notification/event-stream/`, their three member paths are in sorted order in `[workspace].members`, no scaffolder file is hand-written, and `make.architecture` and `make.check_crate_isolation` pass
- **Given** the manifests of the three crates and of `crates/service` after this story, **When** `workspace.dsl` gains the group `Notification/EventStream` with the components `eventStreamDomain`, `eventStreamApplication` and `eventStreamInfrastructure` (technology "Rust crate"), one relationship per workspace-internal dependency edge (composition root to each of the three; domain to `sharedDomain`; application to its domain, `sharedDomain` and `sharedApplication`; infrastructure to its domain and `sharedDomain`) and `eventStreamInfrastructure -> database`, **Then** C2 is done: `make.validate_architecture_model` passes with no missing component and no missing relationship
- **Given** the first crate that depends on each new crate is written in this story, **When** `event-stream-domain`, `event-stream-application` and `event-stream-infrastructure` are added to `[workspace.dependencies]` as path entries and inherited with `.workspace = true`, and each member manifest lists only the crates its code uses in this story, **Then** C3 is done: `make.check_security` and `make.unused_dependencies` pass and no member manifest holds a version or a path
- **Given** the domain crate with `async-trait`, `shared-domain` and `thiserror` only, **When** `EventType::new` is called, **Then** C4 is done: a name of 1 and of 128 characters is accepted, an empty name and a name of 129 characters fail with `InvalidEvent::TypeLength` (code `length`), `User.created`, `1a`, `a..b`, `a.`, `.a`, `a b` and `a/b` fail with `InvalidEvent::TypePattern` (code `pattern`), `a`, `a1`, `user.created` and `user_created-v2.x` are accepted, a property test compares the hand-written matcher with strings generated from the grammar `^[a-z][a-z0-9]*([._-][a-z0-9]+)*$` and with mutations of them, `EventCatalog` (`register`, `find_page`) is usable as `Arc<dyn EventCatalog>`, and `make.architecture`, `make.static_analysis` and `make.complexity` pass
- **Given** the application crate with the module rule written in `lib.rs` (only `http` and `graphql` import `axum`, `utoipa` or `async_graphql`), **When** `error.rs`, `query/list_event_types.rs`, `http.rs`, `http/dto.rs`, `http/handler.rs` (with the extractor-check test) and `graphql.rs` are written, **Then** C5 is done: `ListEventTypesHandler` calls `EventCatalog::find_page` once per query, `EventStreamError::InvalidPage` maps to the 422 problem with one violation on `page` or `itemsPerPage` (code `range`), `EventStreamError::Storage` maps to `Problem::internal`, the REST handler and the GraphQL resolver both call `handler::list_types`, and `make.static_analysis` and `make.tests_with_coverage` pass
- **Given** a migrated test database (`#[sqlx::test]` with the shared migrator), **When** `PgEventCatalog` runs `register` (Q8) and `find_page` (Q9) in `tests/integration/event_catalog.rs`, **Then** C6 is done: a type registered twice, or by two concurrent calls, is stored once, an empty slice writes nothing, a page is ordered by name ascending with the limit and offset of `PageRequest`, an unreachable database yields `EventCatalogError` within the acquire timeout, every trait method only delegates to an inherent method, and `make.tests_with_coverage` passes
- **Given** `tests/support/Cargo.toml` gains `event-stream-domain.workspace = true`, **When** `InMemoryEventCatalog` (`new()`, `failing()`, `fail_times(n)`, `std` only) and the fixture `event_type(rng)` are added with their sibling tests and the module line in `tests/support/src/lib.rs`, **Then** C7 is done: the fake keeps the rules of the port (idempotent register, pages by name), `fail_times(2)` fails exactly the first two calls and then succeeds, and `make.unused_dependencies` and `make.tests_with_coverage` pass
- **Given** no applied migration is edited (`20261110000000_*` is byte-identical), **When** `make.create_migration` runs with `name=create_notification_event_types`, the pair is filled with the table `notification_event_types` of architecture.md, `make.migrate` applies it, the rollback round trip (`make.migrate_rollback`, then `make.migrate` once) is run or recorded `not run (ask rule)`, and `make.sqlx_prepare` writes the metadata of Q8 and Q9, **Then** C8 is done: `tests/integration/migration.rs` finds the table, `notification_event_types_pkey` and `notification_event_types_name_check` (which rejects `User.Created`, an empty name and a name of 129 characters), `.sqlx/` holds two new files committed with the migration, and `make.validate_sqlx_metadata` passes
- **Given** the catalog adapter and the application state, **When** `crates/service` is wired (`Adapters.event_catalog` built on the existing lazy pool in `Adapters::connect`, `ListEventTypesHandler` in `register_queries`, `EventStreamState` in `AppState.event_stream` and as `.data(..)` of the schema, the context router nested under `/api` in `openapi::api_router` with the tag `EventStream`, `Query(ExampleApiResourceQuery, EventStreamQuery)`), **Then** C9 is done: the three `Adapters { .. }` literals carry `InMemoryEventCatalog`, the two contract-pinning tests are updated and none is skipped, the example context and the three health probes are unchanged, and `make.tests_with_coverage` and `make.e2e_tests` pass
- **Given** the wired operation and query, **When** `make.generate_openapi_spec` and `make.generate_graphql_spec` regenerate both snapshots and `features/event_catalog.feature` is added with `tests/e2e/steps/event_catalog.rs` and its module line, **Then** C10 is done: the OpenAPI snapshot holds `GET /api/event-types` (`api_event_types_get`, tag `EventStream`, responses 200 with an array of `EventType`, 400, 422 and `default`), the schema `EventType` (`name` with minLength 1, maxLength 128, the pattern and the example `user.created`) and the version `1.1.0`, the GraphQL snapshot holds `type EventType { name: String! }` and `eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!`, and `make.validate_contract_snapshots`, `make.validate_openapi_spec`, `make.openapi_diff`, `make.graphql_diff` (additions only) and `make.e2e_tests` pass
- **Given** the new endpoint, **When** `docs/requests.http` gets one request for `GET /api/event-types` and one for the GraphQL query, the bounded-context table of `docs/architecture.md` gets the row `event-stream`, and `docs/architecture.md` is added to `.templatesyncignore`, **Then** C11 is done: no variable is added by this story so `docs/environment.md` is unchanged, no decision record is due for the catalog slice, and `make.docs_check` passes
- **Given** a catalog that holds `order.paid`, `user.created` and `user.deleted`, **When** `GET /api/event-types` is sent without parameters, **Then** the answer is 200 with the JSON array `[{"name":"order.paid"},{"name":"user.created"},{"name":"user.deleted"}]`, sorted by name ascending, read with page 1 and 30 items per page
- **Given** the same catalog, **When** the page parameters vary, **Then** `itemsPerPage=1&page=2` returns `[{"name":"user.created"}]`, `page=4&itemsPerPage=1` returns `[]`, `itemsPerPage=1` and `itemsPerPage=100` are accepted, `page=abc` returns 400 as `application/problem+json` with type `about:blank` and the parser text as `detail`, and `page=0`, `itemsPerPage=0` and `itemsPerPage=101` each return 422 with type `/problems/validation-failed` and one violation whose `propertyPath` is `page` or `itemsPerPage`
- **Given** the same catalog, **When** the GraphQL query `eventTypes(page: 2, itemsPerPage: 1) { name }` and the query without arguments are sent, **Then** each returns the same page as the REST operation for the same arguments (defaults 1 and 30), and `itemsPerPage: 101` returns a GraphQL error built from the same 422 problem
- **Given** a catalog fake built with `failing()`, **When** the REST operation and the GraphQL query are called, **Then** REST answers with the 500 problem of `Problem::internal`, GraphQL answers with an error, and neither body holds an event type name or the error chain
- **Given** C1 to C11 are committed as one story commit, **When** the six stage targets run one call each in this order, `make.ci_preflight` (with `base_ref=<base>` and an empty `CONFIG_CHANGE_APPROVED=`), `make.ci_static_analysis`, `make.ci_tests`, `make.ci_contracts` (`base_ref=<base>`), `make.ci_mutation` (`base_ref=<base>`) and `make.ci_delivery`, followed by `make.validate_configuration` with the same two arguments, **Then** C12 is done: every stage is recorded `passed`, or `not run (<reason>)` for a target that is not available in the session (the targets of `make.ci_delivery` that need the production image are named as deferred to CI when the image cannot be built here), 0 protected files changed, `.claude/rust-sdlc.yml` is unchanged and `make.ci_cli` was not run

**Depends on:** none

### Story 1.2: Scaffold internal/cluster

As an operator, I read from `GET /api/cluster` which node this is, its role, the leader and the term, backed by the new context `internal/cluster`. The story is the walking skeleton of the context: the three crates through the scaffolder and the whole lease slice of architecture.md (value objects, the `LeaseStore` port, the one-round elector, the PostgreSQL lease, the status endpoint and the three cluster variables). It depends on story 1.1 by ownership only: both stories write the root `Cargo.toml`, `Cargo.lock`, `workspace.dsl`, the bootstrap, router, OpenAPI and test files of `crates/service`, `tests/support`, `tests/e2e`, `migrations/`, `.sqlx/` and the OpenAPI snapshot.

**Scope:**

- Scaffolder (`make.new_context` with `group=internal context=cluster`): `crates/internal/cluster/{domain,application,infrastructure}`, the three sorted member paths and the three path entries of `[workspace.dependencies]`; `Cargo.lock`.
- Domain: `crates/internal/cluster/domain/src/{lib.rs,error.rs,value_object.rs,port.rs}`, `src/value_object/{node_id.rs,term.rs,leadership.rs}`.
- Application: `crates/internal/cluster/application/src/{lib.rs,elector.rs,http.rs}`, `src/http/{dto.rs,handler.rs}`.
- Infrastructure: `crates/internal/cluster/infrastructure/src/{lib.rs,persistence.rs}`, `tests/integration/{main.rs,migration.rs,lease_store.rs}`.
- Database: the pair `migrations/<timestamp>_create_cluster_leases.{up,down}.sql`, three new files in `.sqlx/` (Q10, Q11, Q12).
- Test support: `tests/support/Cargo.toml`, `tests/support/src/{lib.rs,lease_store.rs}`.
- Composition root: `crates/service/Cargo.toml`, `crates/service/src/{cli.rs,config.rs,bootstrap.rs,http.rs,openapi.rs}` with their tests, the three `Adapters { .. }` literals, `src/openapi/tests.rs`, `tests/integration/check_config.rs`.
- Variables: `.env`, `docs/environment.md` (`NODE_ID`, `CLUSTER_LEASE_TTL`, `CLUSTER_RENEW_INTERVAL`).
- Model, contracts, end to end, documentation: `workspace.dsl`, `.github/openapi-spec/spec.yaml`, `features/cluster.feature`, `tests/e2e/{steps.rs,world.rs}`, `tests/e2e/steps/cluster.rs`, `docs/requests.http`, `docs/architecture.md`, `docs/adr/0013-leader-lease-in-postgresql.md`, `docs/adr/README.md`, `.templatesyncignore`.

**Covers:** FR-19, FR-20 (release and takeover in the store and the elector), FR-21, FR-25 (three variables), NFR-6, NFR-7, NFR-15 (leadership entries), NFR-18, NFR-20.

> Assumption: the three cluster variables are added here and not in story 2.1, because the wiring of the elector needs the node id, the lease time and the call timeout, and the checklist item C11 asks for the rows of new variables. `NODE_ID` gets no line in `.env`, as architecture.md decides.
>
> Assumption: `bootstrap` builds the `Elector` and hands `elector.subscribe()` to `ClusterState`; no loop campaigns yet, so the route reports the initial status until story 5.2 schedules the elector and story 5.3 moves it into `Background`. architecture.md places every loop in `crates/service/src/background.rs`.
>
> Assumption: the decision record `0013-leader-lease-in-postgresql.md` is written here, because the decision is implemented here; the records 0014 to 0017 are written in story 6.5.
>
> Assumption: the GraphQL schema does not change in this story; `make.generate_graphql_spec` is still run and must leave the snapshot unchanged.
>
> Assumption: `crates/service/tests/integration/check_config.rs` is a synchronised file that this story changes, so it is added to `.templatesyncignore` here; architecture.md does not list it.
>
> Assumption: C12 is recorded as in story 1.1.

**Acceptance Criteria:**

- **Given** the names `internal` and `cluster` pass the three name rules against the live tree (re-checked before any write; the group `internal` exists, the context `cluster` does not), **When** the target of `make.new_context` runs with `group=internal context=cluster`, **Then** C1 is done: three crates exist under `crates/internal/cluster/`, their member paths are in sorted order, and `make.architecture` and `make.check_crate_isolation` pass
- **Given** the manifests after this story, **When** `workspace.dsl` gains the group `Internal/Cluster` with the components `clusterDomain`, `clusterApplication` and `clusterInfrastructure`, one relationship per workspace-internal dependency edge (composition root to each of the three; domain to `sharedDomain`; application to its domain, `sharedDomain` and `sharedApplication`; infrastructure to its domain and `sharedDomain`) and `clusterInfrastructure -> database`, **Then** C2 is done: `make.validate_architecture_model` passes and no relationship joins a cluster component and an event-stream component
- **Given** the first user of each new crate is written in this story, **When** `cluster-domain`, `cluster-application` and `cluster-infrastructure` are added to `[workspace.dependencies]` as path entries and inherited with `.workspace = true`, **Then** C3 is done: `make.check_security` and `make.unused_dependencies` pass and no cluster manifest names an `event-stream` crate
- **Given** the domain crate with `async-trait`, `shared-domain` and `thiserror` only, **When** the value objects and the port are written, **Then** C4 is done: `Term::new(0)` fails with `InvalidTerm`, `Term::new(1)` and `Term::new(u64::MAX)` succeed, `NodeId` prints as its UUID, `Leadership::role_of` returns `Leader` for the holder and `Follower` for any other node, `LeaseStoreError` reads `the lease store is unavailable` and keeps its source, `LeaseStore` is usable as `Arc<dyn LeaseStore>`, and `make.architecture`, `make.static_analysis` and `make.complexity` pass
- **Given** the application crate (no command, query or subscriber: the context has no bus message), **When** `elector.rs`, `http.rs`, `http/dto.rs` and `http/handler.rs` are written, **Then** C5 is done: `Elector::new` starts as follower with no leader and no term, `campaign` and `resign` publish through `watch::Sender::send_replace`, the handler reads `state.status.borrow()` without a bus and without awaiting, and `make.static_analysis` and `make.tests_with_coverage` pass
- **Given** a migrated test database and two node ids A and B, **When** `PgLeaseStore` is driven in `tests/integration/lease_store.rs`, **Then** C6 is done: A acquires with term 1, A renews with term 1, B reads `(A, 1)` while the lease of A is valid, after `release` by A the next round of B returns `(B, 2)` and the next round of A reads `(B, 2)`, B takes its own expired lease again with term 2, a lease of 1 ms is taken over without `release` (expiry by the database `now()`, no node clock), `release` by a node that does not hold the lease changes nothing, `release` keeps the row and its term, the lease time is bound as milliseconds, a missing row in the fallback read (Q11) is a `LeaseStoreError`, and `make.tests_with_coverage` passes
- **Given** `tests/support/Cargo.toml` gains `cluster-domain.workspace = true`, **When** `InMemoryLeaseStore` (holder, term, `expire()`, `failing()`, `hanging()`, shared through `Arc`, `std` only) is added with its sibling tests and module line, **Then** C7 is done: two electors on one fake never both get `Leader` for an unexpired lease, the term grows by exactly 1 on a change of holder and stays on a renewal, `hanging()` never completes, and `make.unused_dependencies` and `make.tests_with_coverage` pass
- **Given** no applied migration is edited, **When** `make.create_migration` runs with `name=create_cluster_leases`, the pair is filled with the table of architecture.md, `make.migrate` applies it, the rollback round trip is run or recorded `not run (ask rule)`, and `make.sqlx_prepare` writes the metadata of Q10, Q11 and Q12, **Then** C8 is done: `tests/integration/migration.rs` finds the table `cluster_leases`, `cluster_leases_pkey` and `cluster_leases_term_check` (which rejects term 0), `.sqlx/` holds three new files committed with the migration, and `make.validate_sqlx_metadata` passes
- **Given** the lease adapter, the elector and the variables, **When** `crates/service` is wired (`Adapters.lease_store` on the existing lazy pool, the node id from `NODE_ID` or one call of `uuid_generator.generate()`, `Elector::new` with `ElectorSettings { ttl: CLUSTER_LEASE_TTL, call_timeout: CLUSTER_RENEW_INTERVAL }`, `ClusterState` in `AppState.cluster`, the context router nested under `/api` with the tag `Cluster`), **Then** C9 is done: the three `Adapters { .. }` literals carry `InMemoryLeaseStore`, the OpenAPI pinning test lists the new operation, the node id is the same for the life of the process, and `make.tests_with_coverage` and `make.e2e_tests` pass
- **Given** the wired operation, **When** both snapshot generators run and `features/cluster.feature` is added with `tests/e2e/steps/cluster.rs` and its module line, **Then** C10 is done: the OpenAPI snapshot holds `GET /api/cluster` (`api_cluster_get`, tag `Cluster`, responses 200 with `ClusterStatus` and `default`) and the schemas `ClusterStatus` and `ClusterRole`, the GraphQL snapshot is unchanged, the feature shows 200 with a UUID `nodeId`, `role` `follower`, `leader` `null` and `term` 0 for a node that has finished no election round, and `make.validate_contract_snapshots`, `make.validate_openapi_spec`, `make.openapi_diff`, `make.graphql_diff` (additions only) and `make.e2e_tests` pass
- **Given** the new endpoint and variables, **When** `docs/requests.http` gets the request for `GET /api/cluster`, `docs/architecture.md` gets the row `cluster`, `docs/environment.md` gets the rows of `NODE_ID`, `CLUSTER_LEASE_TTL` and `CLUSTER_RENEW_INTERVAL` and the new cross-field rule, `.env` gets the two variables that have a default, and the decision record 0013 (lease with a term judged by the database clock, Raft rejected) is added and indexed, **Then** C11 is done: `make.docs_check` passes, including its comparison of `docs/environment.md` with the help of `serve`, the compose files and `.env`
- **Given** an `Elector` on `InMemoryLeaseStore` under paused time, **When** `campaign` runs, **Then** a won round returns and publishes `{ role: Leader, leader: Some(own id), term: Some(1) }`, a lost round `{ role: Follower, leader: Some(holder), term: Some(term) }`, one `leadership changed` entry (info, fields `node_id`, `role`, `leader`, `term`) is written when the role or the leader changed and none on a renewal, and a subscriber of `subscribe()` sees every published status
- **Given** an `Elector` that was leader with term 3, **When** the store fails, or hangs longer than `call_timeout`, **Then** the round ends no later than `call_timeout` of virtual time, the status is `{ role: Follower, leader: None, term: Some(3) }`, and one `lease store failed` entry (warn, with the error chain) is written
- **Given** an `Elector`, **When** `resign` is called, **Then** a leader calls `release` once and publishes follower with no leader, a follower makes no store call, and a failing or hanging `release` is logged once and still ends as follower within `call_timeout`
- **Given** the status channel, **When** `GET /api/cluster` is called, **Then** the body is `{"nodeId","role","leader","term"}` with `role` `leader` or `follower` in lower case, `leader` `null` and `term` 0 before the first successful round, the leader id and the term after it, and the answer is 200 also when the database is unreachable
- **Given** the configuration rules, **When** `Config::from_args` and `service check-config` read the three variables, **Then** the defaults are unset, `10s` and `3s`; `NODE_ID=abc` fails with exit status 2 and the reason `must be a UUID`; `CLUSTER_LEASE_TTL` accepts `1s` and `5m` and rejects `999ms` and `5m 1s`; `CLUSTER_RENEW_INTERVAL` accepts `100ms` and rejects `99ms`; with a lease time of `10s` a renew interval of `4999ms` is accepted and `5s` is rejected on `CLUSTER_RENEW_INTERVAL` with the reason `must be shorter than half of CLUSTER_LEASE_TTL`; no message holds the value of a variable
- **Given** C1 to C11 are committed as one story commit, **When** the six stage targets run one call each in this order, `make.ci_preflight` (with `base_ref=<base>` and an empty `CONFIG_CHANGE_APPROVED=`), `make.ci_static_analysis`, `make.ci_tests`, `make.ci_contracts` (`base_ref=<base>`), `make.ci_mutation` (`base_ref=<base>`) and `make.ci_delivery`, followed by `make.validate_configuration` with the same two arguments, **Then** C12 is done: every stage is recorded `passed`, or `not run (<reason>)` for a target that is not available in the session (the targets of `make.ci_delivery` that need the production image are named as deferred to CI when the image cannot be built here), 0 protected files changed, `.claude/rust-sdlc.yml` is unchanged and `make.ci_cli` was not run

**Depends on:** 1.1

## Epic 2: Configuration and local stack

The epic adds the eight remaining variables of FR-25 with their validation and gives the local compose stack the queue the service ingests from. Later stories read `Config` and never the environment.

### Story 2.1: Event and stream configuration variables

As an operator, I set retention, relay polling, data size, the queue, the seeded event types and the subscriber limits through validated variables, and a wrong value stops the start with a message that names the variable and never its value. The story adds `EventsConfig` and `StreamsConfig` to `Config`; nothing reads them yet.

**Scope:**

- `crates/service/src/cli.rs`, `crates/service/src/cli/tests.rs`, `crates/service/src/config.rs`, `crates/service/src/config/tests.rs`, `crates/service/tests/integration/check_config.rs`.
- `.env` (the six variables that have a default), `docs/environment.md` (eight rows).

**Covers:** FR-25, FR-3 (validation of `EVENT_TYPES`), NFR-14.

> Assumption: `.env` gets no line for `EVENT_TYPES` (architecture.md) and none for `EVENT_QUEUE_URL` in this story; story 2.2 adds that line together with the queue it points at.
>
> Assumption: the entries of `EVENT_TYPES` are not trimmed and a repeated name is kept once; a blank entry or an entry with a space is an invalid name. architecture.md only says that every name must be a valid `EventType`.
>
> Assumption: architecture.md says nine variables with a default go into `.env`; its table has eight (two of them written by story 1.2, six here).

**Acceptance Criteria:**

- **Given** none of the eight variables is set, **When** `Config::from_args` runs, **Then** `events` is `{ queue_url: None, retention: 1h, relay_poll_interval: 1s, max_data_bytes: 65536, types: [] }` and `streams` is `{ subscriber_buffer: 256, max_subscribers: 100000, keep_alive: 15s }`
- **Given** the duration variables, **When** a boundary value is set, **Then** `EVENT_RETENTION` accepts `1m` and `30d` and rejects `59s` and `30d 1s`, `EVENT_RELAY_POLL_INTERVAL` accepts `100ms` and `1m` and rejects `99ms` and `61s`, `STREAM_KEEP_ALIVE` accepts `1s` and `5m` and rejects `999ms` and `5m 1s`, and each uses the helper `bounded_duration`
- **Given** the integer variables, **When** a boundary value is set, **Then** `EVENT_MAX_DATA_BYTES` accepts 1 and 1048576 and rejects 0 and 1048577, `SUBSCRIBER_BUFFER` accepts 1 and 65536 and rejects 0 and 65537, `MAX_SUBSCRIBERS` accepts 1 and 1000000 and rejects 0 and 1000001, and `abc` is rejected for each
- **Given** `EVENT_QUEUE_URL`, **When** it is unset, an `http://` URL, an `https://` URL, `ftp://queue` or `not a url`, **Then** the result is `None`, accepted, accepted, rejected and rejected through the existing `endpoint` check
- **Given** `EVENT_TYPES`, **When** it is `user.created,order.paid`, `user.created,user.created`, `user.created,User.Created` or `user.created,,order.paid`, **Then** the result is two types, one type, a rejection and a rejection, and a rejection has the reason `must be a comma-separated list of event type names`
- **Given** any rejected value of the eight variables, **When** `service serve` or `service check-config` starts, **Then** the exit status is 2, one entry per broken variable names the variable, and the rejected value appears in no output line
- **Given** two broken variables at once, **When** the configuration is read, **Then** both problems are reported in one run
- **Given** the new arguments, **When** the help of `serve` is compared with the documentation, **Then** each of the eight variables has a row in `docs/environment.md` with type, default and bounds, the six with a default have a line in `.env` written with that default, and `make.docs_check` passes
- **Given** the story's files on top of story 1.2, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.tests_with_coverage` and `make.docs_check`, **Then** all pass with 100 % line and function coverage of `cli.rs` and `config.rs`, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff` in these files

**Depends on:** 1.2

### Story 2.2: Event queue in the local stack

As a developer, I start the local stack and find the SQS queue `notification-events` in LocalStack with `EVENT_QUEUE_URL` pointing at it, so that the service can ingest without manual setup. The story changes the compose files, adds the LocalStack init script and the `.env` line; no Rust file changes.

**Scope:**

- `docker-compose.yml`, `docker-compose.override.yml`, `docker/localstack/ready.d/create-queues.sh` (new, executable).
- `.env`, `docs/environment.md` (the consumer of `EVENT_QUEUE_URL`), `.templatesyncignore` (the two compose files).

**Covers:** FR-25 (queue of the local stack), FR-15 (the queue the first adapter consumes).

> Assumption: the `.env` line `EVENT_QUEUE_URL=http://${LOCALSTACK_HOST}:${LOCALSTACK_PORT}/000000000000/notification-events` is added here and not in story 2.1, so that the variable never points at a queue the stack does not create.

**Acceptance Criteria:**

- **Given** the compose stack, **When** `localstack` reaches the ready stage, **Then** the script mounted read-only into `/etc/localstack/init/ready.d/` has run `awslocal sqs create-queue --queue-name notification-events` and the queue URL resolves
- **Given** the script ran once, **When** it runs again, **Then** it exits 0 and the queue is unchanged
- **Given** `docker-compose.yml`, **When** the configuration is rendered, **Then** the service `app` has `EVENT_QUEUE_URL: http://localstack:4566/000000000000/notification-events` and the three health probes and the other services are unchanged
- **Given** `docker-compose.override.yml`, **When** the development configuration is rendered, **Then** `EVENT_QUEUE_URL` is reset to null under `app`, so that the development binary reads the value of `.env`
- **Given** `.env` with the new line, **When** `service check-config` runs with the file loaded, **Then** it exits 0
- **Given** the two compose files are synchronised files, **When** the story changes them, **Then** both paths are in `.templatesyncignore`
- **Given** the story's files on top of story 2.1, **When** the repository lint target `lint-repo` (no profile key: shell, YAML and spelling linters), `make.docs_check`, `make.validate_invariants`, `make.secrets_scan` and `make.validate_configuration` run, **Then** all pass, no protected file changed, and the session's `make.lint_code` and `make.unit_tests` still pass because no Rust file changed

**Depends on:** 2.1

## Epic 3: Event model, relay and ingestion core

The epic builds the path of an event from the bus message to the hub's door: the domain model with the log, source and signal ports and their fakes, the relay round with its wake-up, the frame encoders, pruning and seeding, the PostgreSQL log with its signal, and the ingest use case. Every use case is a one-round function that a test drives with fakes and paused time.

### Story 3.1: Event model, log, source and signal ports with their fakes

As a developer of the later stories, I get the domain vocabulary of the event stream (`Sequence`, `Cursor`, `EventData`, `NewEvent`, `Event`) and the ports `EventLog`, `EventSource` and `EventSignal` with in-memory fakes, so that every use case can be written and tested without a database or a queue. The story depends on story 1.2 by ownership of `tests/support/src/lib.rs` and `fixture.rs`; its code needs only story 1.1.

**Scope:**

- `crates/notification/event-stream/domain/src/{lib.rs,error.rs,event.rs,value_object.rs,port.rs}`, `src/value_object/{event_data.rs,sequence.rs,cursor.rs}`, `src/port/{event_log.rs,event_source.rs,event_signal.rs}`, each with its sibling tests.
- `tests/support/src/{lib.rs,fixture.rs,event_log.rs,event_source.rs,event_signal.rs}` with sibling tests.

**Covers:** FR-14 (the port), FR-11 (cursor), FR-5 (sequence as id), NFR-8 (model), NFR-14 (bounds), NFR-19 (fakes).

**Acceptance Criteria:**

- **Given** `Sequence`, **When** it is built, **Then** `Sequence::new(0)` fails with `InvalidEvent::SequenceZero` (code `range`), 1 and `u64::MAX` are accepted, and it prints as its decimal number
- **Given** `Cursor::parse`, **When** it reads text, **Then** `0` is `Cursor::START`, `18446744073709551615` is accepted, and `18446744073709551616`, `-1`, `+1`, ` 1`, `1 `, an empty string, `abc` and `1.0` each fail with `InvalidCursor` (`A cursor must be a decimal unsigned 64-bit integer.`), and `Cursor::from(sequence)` holds the same number
- **Given** `EventData::new(value, max_bytes)`, **When** it is built, **Then** an empty value fails with `DataEmpty` (code `required`), a value of exactly `max_bytes` bytes is accepted and one of `max_bytes + 1` bytes fails with `DataTooLarge` (code `max_bytes`), the size is counted in bytes (a two-byte character that crosses the limit is rejected), and a value with U+0000 fails with `DataContainsNul` (code `does_not_contain`)
- **Given** `InvalidEvent`, **When** `code()` is called on each of the six variants, **Then** it returns `length`, `pattern`, `range`, `required`, `max_bytes` and `does_not_contain`, and the `match` names every variant
- **Given** the three new ports, **When** they are used as `Arc<dyn EventLog>`, `Arc<dyn EventSource>` and `Box<dyn EventSignal>`, **Then** the code compiles (a test holds each as a trait object), and `EventLogError`, `EventSourceError` and `EventSignalError` read `the event log is unavailable`, `the event source is unavailable` and `the event signal is unavailable` and keep their source
- **Given** `InMemoryEventLog`, **When** events are appended and read, **Then** `append` returns the number of new rows and keeps the given order, an id that is already stored is skipped and still consumes a sequence value (a gap), `read_after(cursor, types, limit)` returns only sequences above the cursor in order, filtered by type and cut at the limit, `latest_sequence` is `None` for an empty log, `prune` removes what a test call marked as old and returns the count, `failing()` fails every call and `with_call_budget(n)` fails every call after the n-th
- **Given** `ScriptedEventSource`, **When** it is driven, **Then** `receive` returns the scripted batches in order and then empty batches, `acknowledge` records the receipts, `failing_receive()` and `failing_acknowledge()` fail the named call, and `pending()` gives a `receive` that never completes
- **Given** `ScriptedEventSignal`, **When** it is driven, **Then** `ready(n)` completes n times and is pending afterwards, `pending()` never completes and `failing()` returns `EventSignalError`
- **Given** the fixtures, **When** `event(rng)` and `new_event(rng)` are called with the same seed, **Then** they return the same valid values
- **Given** the story's files on top of story 1.2, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.tests_with_coverage`, **Then** all pass (the domain crate uses only `async-trait`, `shared-domain` and `thiserror`; the fakes use `std` only), the domain files have 100 % line and function coverage, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 1.2

### Story 3.2: Relay round, wake-up and frame encoding

As a node of the cluster, I read new log rows after my cursor once per round, hand each to a publisher in sequence order, wake up on a signal or after the poll interval, and encode an event once for both transports. The story writes `RelayEvents` with the trait `EventPublisher`, `ListenForEvents` and `Frame` with its JSON encoder; the hub that implements the publisher comes in story 4.1.

**Scope:**

- Root `Cargo.toml` (feature `ws` on the `axum` workspace dependency), `Cargo.lock`.
- `crates/notification/event-stream/application/Cargo.toml` (`tokio` and `tracing` as dependencies; `proptest` and `futures-util` as dev-dependencies; the dev features `test-util` and `io-util` of `tokio`).
- `crates/notification/event-stream/application/src/{lib.rs,relay.rs,listen.rs,http.rs}`, `src/http/frame.rs`, each new file with its sibling tests.

**Covers:** FR-5, FR-8, FR-18, NFR-2, NFR-3, NFR-18 (WebSocket through the `ws` feature), NFR-19 (property test).

> Assumption: the frame encoders and the relay are one story. Each needs a manifest change (the `ws` feature and `proptest` for the frame, `tokio` and `tracing` for the relay and the listener), every manifest change rewrites `Cargo.lock`, and only one story per round may write that file; two stories would take two rounds for the hub to start.
>
> Assumption: `error_json` is not written here. Its first caller is the WebSocket session, so story 4.4 adds it to `http/frame.rs`.
>
> Assumption: the SSE half of a frame is asserted by rendering it through `axum::response::sse::Sse` in the test, so `futures-util` enters the manifest here as a dev-dependency; story 4.3 moves it to the dependencies with its first production use.
>
> Assumption: the dev feature `io-util` of `tokio` is declared here although story 4.4 is its first user; a feature flag has no gate, and the manifest keeps one writer per round.
>
> Assumption: the use-case modules are declared `pub mod` in `lib.rs` with a crate-root re-export of the types this story adds; each later story adds its own lines to `lib.rs` or `http.rs` and is chained by ownership.

**Acceptance Criteria:**

- **Given** a new `RelayEvents` (cursor `None`) on a log whose latest sequence is 40, **When** the first `tick` runs, **Then** the cursor becomes 40, nothing is published and the outcome is `Drained`; on an empty log the cursor becomes `Cursor::START`
- **Given** a relay with a cursor and 3 newer events, **When** `tick` runs, **Then** `EventPublisher::publish` is called once per event in sequence order, the cursor is the last sequence, the log is read with no type filter and `PAGE_SIZE` 500, and the outcome is `Drained`
- **Given** 499, 500 and 501 newer events, **When** `tick` runs, **Then** the log is read 1, 2 and 2 times and the outcome is `Drained` in each case; with 10 000 or more newer events the round stops after `MAX_PAGES_PER_TICK` 20 pages with the outcome `More` and exactly 10 000 events published, and the next round continues at the next sequence without a repeat
- **Given** a log that fails, **When** `tick` runs, **Then** a failure of the start read leaves the cursor `None` and publishes nothing, a failure after two published pages returns the error with the cursor at the last published event, and the next successful round publishes the rest once
- **Given** 1 000 calls of `publish` in one round, **When** the calls on the log fake are counted, **Then** the number of reads depends only on the number of pages and not on the publisher (one read per page for all subscribers of the node)
- **Given** `RelayEvents::wait` with a poll interval of 1 s under paused time, **When** `wake` was notified before the call, is notified during the call, or is never notified, **Then** the call returns at once, returns at the notification, or returns after exactly 1 s and not at 999 ms
- **Given** `ListenForEvents::step`, **When** the signal completes, **Then** `wake` holds exactly one stored permit; when the signal fails, one `event signal failed` entry (warn, with the error chain) is written, `wake` is not notified and the step returns after exactly `retry_delay` of virtual time
- **Given** an event with sequence 1042, type `user.created` and data `{"userId":"5f2c"}`, **When** `Frame::encode` runs, **Then** `json` is exactly `{"id":"1042","type":"user.created","data":"{\"userId\":\"5f2c\"}"}` with `id` as a JSON string, and the rendered SSE frame has the lines `id: 1042`, `event: user.created` and `data: {"userId":"5f2c"}`
- **Given** arbitrary UTF-8 data (quotes, backslashes, U+0001 to U+001F, characters outside the Basic Multilingual Plane), **When** `event_json` output is parsed by `serde_json` in a property test, **Then** the three members equal the sequence in decimal, the type and the original data, and `write_json_string` writes `"` and `\` escaped, U+000A, U+000D and U+0009 as `\n`, `\r` and `\t`, the other control characters as `\u00XX` and everything else unchanged
- **Given** data with line breaks, **When** the SSE half is rendered, **Then** `a\nb` gives two `data:` lines, `a\r\nb` gives two and not three, a lone `\r` gives a line break, and the JSON half carries the text unchanged
- **Given** the `ws` feature adds crates to `Cargo.lock`, **When** `make.check_security` runs in the verify step, **Then** it passes for the new lock entries and `architecture.toml` is unchanged
- **Given** the story's files on top of story 3.1, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.tests_with_coverage`, `make.unused_dependencies` and `make.check_security`, **Then** all pass with 100 % line and function coverage of `relay.rs`, `listen.rs` and `http/frame.rs`, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 3.1

### Story 3.3: Prune and seed use cases

As the leader, I delete log rows older than the retention with one call per round, and as any node I register the event types of `EVENT_TYPES` in the catalog. The story writes `PruneEvents` and `SeedEventTypes`; the loops that schedule them and the leader check are in story 5.2. It depends on story 3.2 by ownership of the application `lib.rs`; its code needs stories 1.1 and 3.1.

**Scope:**

- `crates/notification/event-stream/application/src/lib.rs` (modules `prune` and `seed`), `src/prune.rs`, `src/seed.rs`, each with its sibling tests.

**Covers:** FR-12, FR-3 (types of `EVENT_TYPES`).

**Acceptance Criteria:**

- **Given** `PruneEvents` with a retention of 1 h, **When** `tick` runs, **Then** `EventLog::prune` is called once with 1 h and the returned count is passed on
- **Given** a failing log, **When** `PruneEvents::tick` runs, **Then** the `EventLogError` is returned and the use case writes no log entry (the loop of story 5.2 writes it)
- **Given** `SeedEventTypes` with an empty list, **When** `tick` runs, **Then** it succeeds and the catalog fake records zero calls
- **Given** `SeedEventTypes` with two types, **When** `tick` runs twice, **Then** each round makes one `register` call with exactly the two types and the catalog holds each type once
- **Given** a catalog fake built with `fail_times(2)`, **When** `tick` runs three times, **Then** the first two rounds return `EventCatalogError`, the third succeeds and the catalog holds the types
- **Given** the two modules, **When** `make.architecture` and the module rule of `lib.rs` are checked, **Then** neither imports `axum`, `utoipa` or `async_graphql`, and no type of the cluster context appears
- **Given** the story's files on top of story 3.2, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.tests_with_coverage`, **Then** all pass with 100 % line and function coverage of `prune.rs` and `seed.rs`, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 3.2

### Story 3.4: PostgreSQL event log and signal adapters

As a node of the cluster, I append events to one PostgreSQL log whose sequence order equals commit order, read it by cursor, prune it by the database clock and get a wake-up when it changes. The story writes the migration of `notification_events`, `PgEventLog` (Q1 to Q7) and `PgEventSignal`. It depends on story 3.2 by ownership of `Cargo.lock`; its code needs story 3.1.

**Scope:**

- The pair `migrations/<timestamp>_create_notification_events.{up,down}.sql` (the table `notification_events` with its constraints and two indexes), seven new files in `.sqlx/`.
- `crates/notification/event-stream/infrastructure/Cargo.toml` (`uuid`), `Cargo.lock`.
- `crates/notification/event-stream/infrastructure/src/{lib.rs,persistence.rs}`, `src/persistence/{event_log.rs,event_signal.rs}`, `tests/integration/{main.rs,migration.rs,event_log.rs,event_signal.rs}`.

**Covers:** FR-11 (range read), FR-12, FR-17 (idempotent append), FR-18 (signal), NFR-8, NFR-12, NFR-14 (parameterised SQL), NFR-18 (new migration).

> Assumption: this is the third new migration pair of the change (see story 1.1); it holds only the table `notification_events`, its named constraints and its two indexes, exactly as architecture.md writes them.
>
> Assumption: in a parallel round the session writes the adapter, the migration and the tests, and the serialized verify step runs `make.setup_test_db`, `make.sqlx_prepare` and the DB-backed targets. Until that step the crate has no offline metadata for its seven queries, so the story's own `make.lint_code` result is taken after `make.sqlx_prepare`.

**Acceptance Criteria:**

- **Given** the new pair created with `make.create_migration` (`name=create_notification_events`), **When** `tests/integration/migration.rs` inspects a migrated database, **Then** it finds `notification_events_pkey`, `notification_events_id_key`, `notification_events_event_type_check` (rejects `User.Created` and a name of 129 characters), `notification_events_data_check` (rejects empty data), `notification_events_event_type_sequence_idx` and `notification_events_recorded_at_idx`, and the earlier migration files are byte-identical
- **Given** `PgEventLog::append` with a batch of three new events, **When** it runs, **Then** it returns 3, the rows have increasing sequences in the given order, and Q1 (`pg_advisory_xact_lock` with `APPEND_LOCK_KEY` 4004000001), Q2 and Q3 run in one transaction
- **Given** an empty slice, a batch with the same id twice, and a batch whose ids are all stored, **When** `append` runs, **Then** it returns 0 without opening a transaction, 1 with one row stored, and 0 with no new row; a later append gets a higher sequence than any before it, and gaps are allowed
- **Given** rows with sequences 1 to 10 of two types, **When** `read_after` runs, **Then** cursor 0 with limit 3 returns 1, 2, 3, cursor 3 returns from 4, a type filter returns only rows of the listed types (Q5) and no filter returns all (Q4), cursor 10 returns nothing, and a cursor above `i64::MAX` is bound as `i64::MAX` and returns nothing
- **Given** an event whose `occurred_at` has microseconds, **When** it is appended and read back, **Then** the time is equal to the microsecond, and an `occurred_at` before the Unix epoch makes `append` return `EventLogError` with nothing stored
- **Given** an empty log and a log with rows, **When** `latest_sequence` runs, **Then** it returns `None` and the highest sequence
- **Given** rows of which some have `recorded_at` older than the retention, **When** `prune(retention)` runs, **Then** exactly the older rows are deleted by the database `now()`, the count is returned, a younger row is never deleted, and the next append gets a sequence above every deleted one
- **Given** two tasks that append concurrently and a reader that pages by `sequence > cursor`, **When** all three run until 200 events are stored, **Then** the reader sees every id exactly once in increasing sequence order and skips no row
- **Given** a `PgListener` on `notification_events`, **When** an append commits, **Then** one notification arrives on the channel `NOTIFY_CHANNEL`
- **Given** `PgEventSignal` on its own pool, **When** `changed` is called, **Then** the first call creates the listener and completes after an append, five appends before the next call give one completion and the call after it stays pending (the test bounds it with a timeout), an unreachable database gives `EventSignalError` and drops the listener, and the next call against a reachable database works again
- **Given** an unreachable database, **When** any `PgEventLog` method runs, **Then** it returns `EventLogError` within the acquire timeout
- **Given** the adapters, **When** `make.architecture` runs, **Then** the infrastructure crate has no `tokio` dependency outside its dev-dependencies and the adapters contain no sleep, retry, spawn or select
- **Given** the story's files on top of stories 3.1 and 3.2, **When** the session runs `make.format`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.setup_test_db`, `make.sqlx_prepare` (seven new files committed), `make.validate_sqlx_metadata`, `make.lint_code`, `make.integration_tests`, `make.tests_with_coverage` and `make.unused_dependencies`, **Then** all pass with 100 % line and function coverage of `event_log.rs` and `event_signal.rs`, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 3.2

### Story 3.5: Ingest use case and envelope validation

As the leader, I turn a batch of bus messages into stored events: each message is validated, the types are registered, the valid events are appended once, and every message is acknowledged only after that. The story writes `IngestEvents` with `receive` and `process` and the envelope rules; it knows nothing about leadership or SQS. It depends on story 3.3 by ownership of the application `lib.rs` and on story 3.4 by ownership of `Cargo.lock`; its code needs stories 1.1 and 3.1.

**Scope:**

- `crates/notification/event-stream/application/Cargo.toml` (`humantime`), `Cargo.lock`.
- `crates/notification/event-stream/application/src/lib.rs` (module `ingest`), `src/ingest.rs`, `src/ingest/envelope.rs`, each with its sibling tests.

**Covers:** FR-3 (types of ingested events), FR-14, FR-15 (envelope rules), FR-16, FR-17, NFR-8, NFR-14, NFR-15.

**Acceptance Criteria:**

- **Given** `IngestEvents::receive`, **When** the source returns a batch or fails, **Then** the batch is returned unchanged or `IngestError::Source` is returned
- **Given** an empty batch, **When** `process` runs, **Then** it returns the default `IngestReport` and calls neither the clock, the catalog, the log nor `acknowledge`
- **Given** a message with `event-type` `user.created`, `event-id` a UUID, `event-time` `2026-11-13T09:30:00Z` and a body, **When** `process` runs, **Then** one `NewEvent` with exactly these values is appended, the catalog registered `user.created`, the receipt is acknowledged, and the report is `{ received: 1, appended: 1, invalid: 0 }`
- **Given** a message without `event-id` and without `event-time`, **When** `process` runs, **Then** the id is the message id parsed as a UUID and `occurred_at` is `Clock::now()`, read once per batch
- **Given** messages that break one rule each, **When** `process` runs, **Then** a missing `event-type` is rejected as `MissingType`, the type `User.Created` as `Type`, an `event-id` or a defaulted message id that is not a UUID as `Id`, the times `2026-11-13T09:30:00+02:00` and `yesterday` as `Time`, and an empty body, a body of `max_data_bytes + 1` bytes and a body with U+0000 as `Data`, while a body of exactly `max_data_bytes` bytes is accepted
- **Given** a rejected message, **When** the log entries are read, **Then** there is exactly one `inbound message rejected` entry (warn) with the fields `message_id` and `reason`, and no entry holds the body or the value of an attribute
- **Given** a batch of three valid messages (two of type `a.b`, one of type `c.d`) and one invalid message, **When** `process` runs, **Then** the catalog gets one `register` call with the two distinct types, the log gets one `append` call with the three valid events in batch order, the four receipts (valid and rejected) are acknowledged in one call, and the report is `{ received: 4, appended: 3, invalid: 1 }`
- **Given** a batch with only invalid messages, **When** `process` runs, **Then** neither the catalog nor the log is called and every receipt is acknowledged
- **Given** the recorded call order of the fakes, **When** a valid batch is processed, **Then** the order is register, append, acknowledge
- **Given** a catalog that fails, or a log that fails, **When** `process` runs, **Then** `IngestError::Catalog` or `IngestError::Log` is returned and nothing is acknowledged, so the bus redelivers the batch
- **Given** an acknowledge that fails after the append, **When** the same batch is processed again, **Then** the first call returns `IngestError::Source`, the second appends 0 rows (`appended: 0`), stores no second copy and acknowledges
- **Given** an event id that is already in the log, **When** its message is processed, **Then** no new sequence is given, `appended` is 0 and the message is acknowledged
- **Given** the two modules, **When** `make.architecture` and the module rule are checked, **Then** no file of the domain or application crate names SQS, and `ingest` imports neither `axum`, `utoipa` nor `async_graphql`
- **Given** the story's files on top of stories 3.3 and 3.4, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.tests_with_coverage` and `make.unused_dependencies`, **Then** all pass with 100 % line and function coverage of `ingest.rs` and `ingest/envelope.rs`, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 3.3, 3.4

## Epic 4: Subscriptions and streaming transports

The epic delivers events to clients: the in-process hub with bounded queues, the subscription that replays from the log and then switches to live delivery, the SSE route and the WebSocket route with run-time commands. After the epic a client can subscribe, resume and be evicted; events still enter only through tests until Epic 5 schedules the loops.

### Story 4.1: Hub with bounded queues, eviction and the subscriber cap

As a node, I fan one event out to the subscribers of its type only, never wait for a client, evict a subscriber whose queue is full and refuse new subscribers at the cap. The story writes `Hub`, `Subscriber`, `HubSettings`, `HubError` and `Delivery` and implements `EventPublisher` for the hub.

**Scope:**

- `crates/notification/event-stream/application/src/http.rs` (module `hub` and its re-exports), `src/http/hub.rs` with its sibling tests.

**Covers:** FR-22, FR-23, FR-24 (close), NFR-1, NFR-2, NFR-9, NFR-10.

**Acceptance Criteria:**

- **Given** subscribers of the types `a` and `b`, **When** a frame of type `a` is delivered, **Then** only the subscribers of `a` receive it, in subscriber-id order, `Delivery.delivered` is their number, and the queue of every subscriber of `b` is untouched
- **Given** 10 000 subscribers of type `a` and 10 000 of type `b`, **When** one frame of type `a` is delivered, **Then** exactly 10 000 queues hold the same `Arc<Frame>` (pointer-equal, encoded once) and `deliver` is a synchronous function that awaits nothing
- **Given** an event whose type has no subscriber, **When** `EventPublisher::publish` is called, **Then** it returns without delivering, and a later subscriber of that type does not receive the earlier event
- **Given** a subscriber with a buffer of 2 that reads nothing, **When** frames are delivered, **Then** the first two are queued without eviction, the third call reports `evicted: 1` and removes the subscriber from every type entry and from the count, the evicted subscriber still receives the two queued frames and then `None` with `hub_closed()` false, and a second subscriber of the same type that reads after each delivery receives all three frames
- **Given** a hub with `max_subscribers` 2, **When** subscribers register, **Then** the second succeeds, the third fails with `HubError::AtCapacity` (`the node is at its subscriber capacity`), and after one subscriber is dropped or evicted the next `register` succeeds
- **Given** a registered subscriber, **When** it is dropped, **Then** `subscriber_count` falls by one and its senders are gone from every type entry
- **Given** `Subscriber::subscribe` and `unsubscribe`, **When** the type set changes, **Then** frames of an added type arrive, frames of a removed type are no longer queued, and adding a held type or removing an absent type changes nothing
- **Given** a hub with subscribers and queued frames, **When** `close` is called, **Then** each subscriber receives its queued frames and then `None` with `hub_closed()` true, `subscriber_count` is 0, `is_closed()` is true, and a later `register` fails with `HubError::Closed` (`the node is shutting down`)
- **Given** a subscriber whose receiver was dropped without the hub noticing, **When** a frame of its type is delivered, **Then** the closed sender is counted as evicted and removed
- **Given** the registry mutex was poisoned by a panicking thread, **When** `register` and `deliver` are called, **Then** both work on the recovered state, and the `Debug` output of the hub shows counts only
- **Given** the story's files on top of story 3.2, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.tests_with_coverage`, **Then** all pass (no `HashMap` is iterated, no lock is held across an await), `http/hub.rs` has 100 % line and function coverage, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 3.2

### Story 4.2: Subscription: replay, then live

As a client that reconnects with a cursor, I receive every retained event of my types after it, in order and once, and then live events without a gap or a duplicate. The story writes `Subscriptions`, `Subscription`, `SubscriptionRequest`, `SubscribeError` with its problem mapping, `EndReason` and `TooManyTypes`.

**Scope:**

- `crates/notification/event-stream/application/src/http.rs` (module `subscription` and its re-exports), `src/http/subscription.rs` with its sibling tests.

**Covers:** FR-9 (set of at most 100 types), FR-11, FR-22 (end reason), FR-23 (problem), NFR-3, NFR-8, NFR-9, NFR-11, NFR-15.

**Acceptance Criteria:**

- **Given** a request without a cursor and a log fake built with `failing()`, **When** `Subscriptions::open` runs, **Then** it succeeds, the log records zero calls, and `next` returns live frames only
- **Given** a request with a cursor, **When** `open` runs, **Then** the subscriber is registered in the hub before `latest_sequence` is read, and a failing log returns `SubscribeError::Log` with the subscriber count back at its earlier value
- **Given** a hub at capacity or closed, **When** `open` runs, **Then** it returns `SubscribeError::Hub`, the problem is 503 with type `about:blank` and the error text as `detail`, one `subscription rejected` entry (warn, field `reason`) is written, and `SubscribeError::Log` maps to the 500 problem of `Problem::internal`
- **Given** a log with 1 200 events of the subscribed types and a cursor of 0, **When** `next` is called until the replay ends, **Then** the frames arrive in sequence order through three reads of `REPLAY_PAGE` 500 (500, 500, 200), `pending` never holds more than one page, and with exactly 500 events a second read returns nothing and the subscription goes live
- **Given** an event that the relay publishes after the registration and that a replay page also returns, **When** the frames are read, **Then** the event is delivered exactly once
- **Given** an event that was committed before the registration with a sequence above the cursor, **When** the frames are read, **Then** it comes from the replay
- **Given** a relay that lags and publishes a sequence that is not above the client's cursor, **When** the frames are read, **Then** that frame is dropped
- **Given** a cursor of `u64::MAX` and a latest sequence of 10, **When** the event with sequence 11 is published, **Then** it is delivered (the floor is the latest sequence, not the cursor)
- **Given** any subscription, **When** its frames are read to the end, **Then** the sequences are strictly increasing and `last_sequence` is the last one returned
- **Given** a replay page that fails, an eviction and a closed hub, **When** `next` returns `None`, **Then** `end_reason` is `LogFailed`, `Evicted` and `HubClosed`
- **Given** a subscription with 99 types, **When** `subscribe` adds types, **Then** a union of 100 succeeds, a union of 101 fails with `TooManyTypes` (`a connection subscribes to at most 100 event types`) and leaves the set and the hub entry unchanged
- **Given** a subscription with a queued frame of type `a`, **When** `unsubscribe` removes `a`, **Then** the queued frame is not returned and no later frame of `a` is returned, and `type_count` falls by one
- **Given** a `next` future that is dropped while a page is being read, **When** `next` is called again, **Then** no frame is lost and none is repeated
- **Given** the story's files on top of story 4.1, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.tests_with_coverage`, **Then** all pass with 100 % line and function coverage of `http/subscription.rs`, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 4.1

### Story 4.3: SSE route and its wiring

As a browser client, I open `GET /api/events/stream?types=a,b` and receive every event of the listed types as an SSE frame with `id`, `event` and `data`, resume with `Last-Event-ID`, and get a problem for invalid input. The story writes the handler `stream` with the shared request parsing and wires the hub, the subscriptions and the event log into `crates/service`. It depends on story 3.5 by ownership of the application manifest and on story 2.1 for the three stream variables; the loops that feed the hub come in story 5.2.

**Scope:**

- `crates/notification/event-stream/application/Cargo.toml` (`futures-util` moves to the dependencies).
- `crates/notification/event-stream/application/src/http.rs` (`EventStreamState` gains `subscriptions` and `settings`; `StreamSettings`; the route `/events/stream` added with a plain `.route(..)`), `src/http/dto.rs` (`StreamParams`, `parse_types`, `parse_cursor`), `src/http/handler.rs` (`stream`), with their tests.
- `crates/service/src/bootstrap.rs` (`Adapters.event_log`, `Hub::new`, `Subscriptions::new`, `StreamSettings`), `crates/service/src/bootstrap/tests.rs`, `crates/service/src/http/router/tests.rs`, `crates/service/src/openapi/tests.rs`, `crates/service/tests/integration/panic.rs`.

**Covers:** FR-4, FR-5, FR-6, FR-11, FR-13, FR-22, FR-23, NFR-11, NFR-14, NFR-15, NFR-20.

> Assumption: this story adds `Adapters.event_log` and builds the hub and the subscriptions in `bootstrap`, because the new fields of `EventStreamState` change the struct literal of the composition root in the same commit. The fields `event_source` and `event_signal` are added by story 5.3.
>
> Assumption: `StreamSettings` is written with both fields here (`keep_alive` from `STREAM_KEEP_ALIVE`, `max_command_bytes` from `HTTP_MAX_BODY_BYTES`), so that story 4.4 needs no change in `crates/service`.

**Acceptance Criteria:**

- **Given** valid types, **When** `GET /api/events/stream?types=a.b,c.d` is sent, **Then** the head is 200 with `content-type: text/event-stream` and `cache-control: no-cache` and the body stays open
- **Given** an open stream for `a.b`, **When** an event of `a.b` and one of `x.y` are published to the hub, **Then** exactly one frame arrives with `id` the sequence in decimal, `event` `a.b` and the data, and frames of one stream have strictly increasing ids
- **Given** the parameter `types`, **When** it is missing, empty, `a,,b`, `User.Created`, a name of 129 characters, 101 distinct names, 100 distinct names, or 100 distinct names with repeats, **Then** the answers are 400 (type `about:blank`, `detail` names `types`), 422 with code `required`, 422 `required`, 422 `pattern`, 422 `length`, 422 `max_items`, 200 and 200, every 422 has type `/problems/validation-failed` and one violation with `propertyPath` `types`, and no stream is opened for an error
- **Given** a cursor, **When** it comes as the header `Last-Event-ID: 5`, as `lastEventId=5`, or as both with different values, **Then** the replay starts after 5, after 5, and after the header value
- **Given** a malformed cursor (`abc`, `-1`, `18446744073709551616`), **When** it comes in the header or in the query, **Then** the answer is 400 with type `about:blank` and a `detail` that names `Last-Event-ID` or `lastEventId`
- **Given** a log with retained events of the subscribed types, **When** the stream is opened with a cursor of 0, **Then** every retained event arrives in order before the first live event and none arrives twice
- **Given** an open stream under paused time with `STREAM_KEEP_ALIVE` 15 s, **When** no frame is written for 15 s, **Then** one comment line `:` followed by an empty line is written, it carries no `id`, and a frame written at 10 s moves the next comment to 25 s
- **Given** a hub at its cap, a closed hub, a failing log with a cursor, and a failing log without a cursor, **When** the stream is requested, **Then** the answers are 503 (type `about:blank`, `detail` says the node is at capacity), 503, the 500 problem and 200
- **Given** a subscriber that is evicted, **When** its body is read, **Then** it ends after the frames already queued and one `subscriber evicted` entry (warn, `transport` `sse`, `types` as a count, `last_sequence`) is written; when the hub is closed the body ends and no such entry is written
- **Given** any request of this story with an invalid event type name, **When** the log entries are read, **Then** no entry holds the name, the data of an event or the cursor text
- **Given** the assembled router with `HTTP_REQUEST_TIMEOUT` 10 s under paused time, **When** a stream is open for 60 s, **Then** it is still open and delivers a frame (the request budget bounds only the response head)
- **Given** the OpenAPI document, **When** the pinning test and `make.validate_contract_snapshots` run, **Then** the document has no path `/api/events/stream` and both snapshots are unchanged
- **Given** `crates/service`, **When** the wiring is read, **Then** `Adapters::connect` builds `PgEventLog` on the existing lazy pool without touching the network, the hub gets `max_subscribers` from `MAX_SUBSCRIBERS` and `buffer` from `SUBSCRIBER_BUFFER`, the three `Adapters { .. }` literals carry `InMemoryEventLog`, and the route is served through the same eight middleware layers as the other routes
- **Given** the story's files on top of stories 2.1, 3.4, 3.5 and 4.2, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.tests_with_coverage`, `make.e2e_tests`, `make.validate_contract_snapshots` and `make.unused_dependencies`, **Then** all pass with 100 % line and function coverage of the changed files, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 2.1, 3.4, 3.5, 4.2

### Story 4.4: WebSocket session and route

As a client, I upgrade `GET /api/events/ws` to a WebSocket, subscribe and unsubscribe at run time with JSON commands, receive matching events as JSON text frames and get an error frame for an invalid command while the connection stays open. The story writes the socket-free `Session`, the pump `socket::serve`, the handler `ws` and the real-socket integration test. It depends on story 4.3 because both routes share `http.rs`, `http/dto.rs` and `http/handler.rs` and the request parsing of that story.

**Scope:**

- `crates/notification/event-stream/application/src/http.rs` (modules `session` and `socket`, the route `/events/ws` with a plain `.route(..)`), `src/http/dto.rs` (`WsParams`, `Command`), `src/http/handler.rs` (`ws`), `src/http/frame.rs` (`error_json`), `src/http/session.rs`, `src/http/socket.rs`, with their tests.
- `crates/notification/event-stream/application/tests/integration/{main.rs,support.rs,ws.rs}` (new).

**Covers:** FR-7, FR-8, FR-9, FR-10, FR-11 (cursor on the WebSocket route), FR-13, FR-22, FR-23, FR-24 (close code 1001), NFR-14, NFR-15, NFR-19.

> Assumption: the integration target of this crate needs no database, but it runs with the other `integration` targets under `make.integration_tests` in the verify step, where coverage and mutation look for the two lines only a socket reaches (`on_upgrade` and `impl Connection for WebSocket`).

**Acceptance Criteria:**

- **Given** a request through `oneshot` (no upgrade state), **When** `GET /events/ws` is sent with valid parameters, with `types=User.Created`, with `lastEventId=abc`, at the subscriber cap, and with a cursor on a failing log, **Then** the answers are the 426 problem, 422 with a violation on `types`, 400 naming `lastEventId`, 503 and 500, so parsing and opening happen before the upgrade is looked at
- **Given** a request without `types`, with `types=a.b,c.d`, and with `types=`, **When** it is parsed, **Then** the subscription starts with 0 types, with 2 types, and the request is answered with 422 code `required`
- **Given** `Session::on_message` with a text frame `{"action":"subscribe","types":["a.b"]}` and later `{"action":"unsubscribe","types":["a.b"]}`, **When** events of `a.b` are published, **Then** the first command has no answer and later events of `a.b` are returned by `next_frame`, the second has no answer and no later event of `a.b` is returned, also not one that was queued, and a run-time subscribe causes no log read
- **Given** a session, **When** it gets a subscribe for a held type or an unsubscribe for an absent type, **Then** the result is `Continue` and the set is unchanged
- **Given** each invalid input, **When** `on_message` runs, **Then** the reply is one text frame `{"error":"<reason>"}` with the reason `invalid command` for text that is not JSON, an unknown `action`, an unknown member or missing `types`; `types must hold 1 to 100 event type names` for an empty list or 101 names; `invalid event type name` for `User.Created`; `a connection subscribes to at most 100 event types` when the union would be 101; `binary frames are not supported` for a binary frame; `the command is too large` for a text frame of `max_command_bytes + 1` bytes; and in every case the type set is unchanged and the result is not `Close`
- **Given** a text frame of exactly `max_command_bytes` bytes that holds a valid command, **When** `on_message` runs, **Then** the command is applied; and the handler sets the transport limit to twice `max_command_bytes`
- **Given** `on_message`, **When** it gets `Ping`, `Pong` and `Close`, **Then** the results are `Continue`, `Continue` and `Close`, and the `match` names every variant of `Message`
- **Given** `socket::serve` with a scripted connection under paused time, **When** a frame is ready, a command arrives and the keep-alive interval of 15 s elapses, **Then** the frame is transmitted as the text `{"id":"..","type":"..","data":".."}`, the command is applied or answered, and one ping is transmitted at 15 s and one at 30 s
- **Given** `socket::serve`, **When** the subscription ends by eviction, by a closed hub and by a failed replay page, **Then** the queued frames are transmitted first and the close frame carries 1013, 1001 and 1011, and only the eviction writes one `subscriber evicted` entry (warn, `transport` `websocket`, `types` as a count, `last_sequence`)
- **Given** `socket::serve`, **When** the client sends a close frame, `receive` returns `None`, or `transmit` fails, **Then** the task ends, the subscriber is removed from the hub (its slot is free), and `receive` is not called again after it returned `None`
- **Given** the context router served by `axum::serve` on a loopback listener with a real `Hub` and `InMemoryEventLog`, **When** the hand-written client of `tests/integration/support.rs` sends the handshake, a masked subscribe command and reads frames, **Then** the answer is 101, a published event arrives as one text frame, an invalid command is answered with an error frame on the same connection, and `lastEventId=0` replays the retained events before live ones
- **Given** the OpenAPI document, **When** the pinning test runs, **Then** it has no path `/api/events/ws`
- **Given** the story's files on top of story 4.3, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.integration_tests` and `make.tests_with_coverage`, **Then** all pass with 100 % line and function coverage of `session.rs`, `socket.rs`, `handler.rs`, `dto.rs` and `frame.rs`, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 4.3

## Epic 5: Bus adapter and background composition

The epic connects the pieces into a running node: the SQS adapter behind the `EventSource` port, the six background loops with leader gating in the composition root, and the wiring of the adapters, the application and the shutdown order in `serve`. After the epic an event sent to the queue reaches a subscriber of any node.

### Story 5.1: SQS event source adapter

As the leader, I long-poll the SQS queue for a batch of at most ten messages and delete the processed ones, behind the `EventSource` port, so that a Kafka adapter can replace this one without a change in the domain or application crates. The story writes `SqsEventSource` and its settings. It depends on story 4.3 by ownership of `Cargo.lock` and the manifests; its code needs story 3.1.

**Scope:**

- `crates/notification/event-stream/infrastructure/Cargo.toml` (`aws-sdk-sqs`; dev `aws-sdk-sqs` with `test-util` and `aws-smithy-mocks`; `thiserror` if the adapter defines an error source), `Cargo.lock`.
- `crates/notification/event-stream/infrastructure/src/lib.rs` (module `messaging`), `src/messaging.rs` with its sibling tests, `tests/integration/{main.rs,support.rs,event_source.rs}`.
- `workspace.dsl` (`system.service.eventStreamInfrastructure -> messageBroker`).

**Covers:** FR-14, FR-15, NFR-11 (queue outage is an error of one round), NFR-14 (10 messages per batch).

**Acceptance Criteria:**

- **Given** settings `{ queue_url, max_messages: 10, wait: 5 s }` and a mocked client, **When** `receive` runs, **Then** one `ReceiveMessage` call is made with the queue URL, `MaxNumberOfMessages` 10, `WaitTimeSeconds` 5 and the three attribute names `event-type`, `event-id` and `event-time`
- **Given** a received message with all three attributes and one with none, **When** it is mapped, **Then** `InboundMessage` holds the receipt handle, the message id, the three attribute strings or `None`, and the body, all unvalidated
- **Given** an empty poll, a message without a receipt handle and a failing call, **When** `receive` runs, **Then** it returns an empty list, an `EventSourceError` and an `EventSourceError`
- **Given** `acknowledge`, **When** it gets 0, 1, 10, 11 and 21 receipts, **Then** it makes 0, 1, 1, 2 and 3 `DeleteMessageBatch` calls, each with at most 10 entries whose ids are `"0"` to `"9"` and whose receipt handles are the given ones in order
- **Given** a `DeleteMessageBatch` answer with HTTP 200 and a non-empty `failed` list, **When** `acknowledge` runs, **Then** it returns an `EventSourceError` that carries the number of failed entries
- **Given** LocalStack and one new queue per test, **When** a message with the envelope of architecture.md is sent, **Then** `receive` returns it with the same attributes and body, `acknowledge` removes it (the next `receive` is empty), and `receive` on a missing queue returns an `EventSourceError`
- **Given** the adapter, **When** `make.architecture` runs, **Then** it passes: the adapter has no sleep, retry or timeout of its own, and no domain or application file names SQS
- **Given** the model, **When** `workspace.dsl` gains the relationship from `eventStreamInfrastructure` to `messageBroker` ("Receives and deletes the messages of the event queue"), **Then** `make.validate_architecture_model` passes
- **Given** the story's files on top of stories 3.1 and 4.3, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.integration_tests`, `make.tests_with_coverage`, `make.unused_dependencies`, `make.check_security` and `make.validate_architecture_model`, **Then** all pass with 100 % line and function coverage of `messaging.rs`, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 3.1, 4.3

### Story 5.2: Background loops with leader gating

As a node, I run the election, seed, listen, relay, ingest and prune rounds on their schedule, ingest and prune only while I am leader, and stop every loop within one pause at shutdown, then release the lease and close the hub. The story writes `crates/service/src/background.rs` with `Schedule`, `Background`, `Running` and the six loops; `bootstrap` and `serve` start using it in story 5.3. It depends on story 4.3 by ownership of `crates/service` and on story 2.2 by ownership of `.templatesyncignore`.

**Scope:**

- `crates/service/src/lib.rs` (module `background`), `crates/service/src/background.rs`, `crates/service/src/background/tests.rs`.
- `.templatesyncignore` (`crates/service/src/lib.rs`).

**Covers:** FR-3 (seed retry), FR-12 (leader only), FR-17 (leader only), FR-18 (relay schedule), FR-19 (campaign schedule), FR-20 (release at shutdown), FR-24, NFR-6, NFR-10, NFR-11, NFR-15.

> Assumption: `crates/service/src/lib.rs` is a synchronised file that gets the module line of this story, so it is added to `.templatesyncignore` here; architecture.md does not list it.
>
> Assumption: the supervisor is two stories. This one holds the loops and `Running::shutdown`, tested with fakes and paused time; story 5.3 builds `Background` from the adapters and calls it from `serve`.

**Acceptance Criteria:**

- **Given** `Background::spawn` with a renew interval of 3 s under paused time, **When** 7 s pass, **Then** the lease store fake recorded campaigns at 0 s, 3 s and 6 s
- **Given** the seed loop with a catalog fake built with `fail_times(2)` and a retry delay of 1 s, **When** time advances, **Then** `seed.tick` runs at 0 s, 1 s and 2 s, each failed round writes one `catalog seed failed` entry (error, with the error chain), and the loop ends after the first success and makes no further call
- **Given** the listen and relay loops, **When** the signal fake completes, **Then** `listen.step` notifies `wake` and the relay runs its next `tick` in the same virtual instant; with a pending signal the relay still runs every poll interval; after an outcome `More` the next round starts without waiting
- **Given** a relay round that fails, **When** the loop continues, **Then** exactly one `relay failed` entry (error, with the error chain) is written per failed round and the next round runs after the pause
- **Given** `ingest` is `None`, **When** the loops are spawned, **Then** no ingest task receives and the other five loops run
- **Given** an elector that is follower, **When** time advances, **Then** the source fake records zero `receive` calls and the log fake zero `prune` calls
- **Given** the status changes to leader, **When** the ingest loop wakes, **Then** it calls `ingest.receive` and then `ingest.process` for each batch, and the prune loop calls `prune.tick` every prune interval; a failed round writes one `ingestion failed` or `prune failed` entry (error) and the ingest loop pauses the retry delay
- **Given** a leader whose lease store starts to fail, **When** the next campaign publishes follower, **Then** the ingest loop finishes the batch in progress and makes no further `receive` call, and the prune loop makes no further call
- **Given** running loops with a batch in `ingest.process`, **When** `Running::shutdown` is called, **Then** the stop flag ends every pause, a pending `receive` is cancelled, the batch in progress is processed to its end (appended and acknowledged) or is not acknowledged at all, every task is joined, and the recorded order is: loops stopped, then `release` on the lease store, then `hub.close()`, with no campaign after the release
- **Given** a lease store fake, a log fake and a signal fake that fail for several rounds and then work again (a database outage), **When** time advances through the outage and beyond, **Then** during the outage the node is follower, the source records no `receive` call and each relay round fails once per poll interval with the cursor unchanged, and afterwards the next campaign makes the node leader, ingestion resumes and the relay publishes every event appended meanwhile once and in order
- **Given** a loop task that panicked, **When** `Running::shutdown` joins it, **Then** one `background task failed` entry (error, field `task`) is written and the other loops were not affected
- **Given** `background.rs`, **When** `make.architecture` runs, **Then** the leader check (`role == Role::Leader` on `elector.subscribe()`) is only in this module and no `event-stream` crate depends on a `cluster` crate
- **Given** the story's files on top of stories 1.2, 2.2, 3.3, 3.5, 4.1 and 4.3, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.tests_with_coverage`, **Then** all pass (every loop has an awaited pause, every arm of each `tokio::select!` is taken by a test), `background.rs` has 100 % line and function coverage, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 1.2, 2.2, 3.3, 3.5, 4.1, 4.3

### Story 5.3: Adapters, application wiring and graceful shutdown

As an operator, I start one binary that campaigns, relays, ingests when a queue is configured, and on SIGTERM stops its loops, releases the lease, closes every stream and exits with status 0 inside the grace period. The story adds the adapters `event_source` and `event_signal`, builds `Application { router, background }` and changes `serve` to spawn and shut down the background tasks. It depends on story 2.2 by ownership of `.templatesyncignore`.

**Scope:**

- `crates/service/src/bootstrap.rs` (`Adapters.event_source`, `Adapters.event_signal`, `Adapters::connect`, `Application`, `application()`, `router()` kept, the constants `QUEUE_WAIT`, `QUEUE_BATCH`, `PRUNE_INTERVAL`, `RETRY_DELAY`), `crates/service/src/bootstrap/tests.rs`, `crates/service/src/http/router/tests.rs`, `crates/service/src/command/serve.rs`.
- `crates/service/tests/integration/{main.rs,support.rs,panic.rs,bootstrap.rs,serve.rs,shutdown.rs}` (`shutdown.rs` is new).
- `.templatesyncignore` (`crates/service/src/command/serve.rs`, `crates/service/tests/integration/main.rs`, `crates/service/tests/integration/support.rs`).

**Covers:** FR-15 (no queue, no ingestion), FR-18, FR-20, FR-21 (node id), FR-24, FR-25 (read once in the composition root), NFR-5, NFR-6, NFR-11, NFR-12, NFR-15.

> Assumption: the tests of the open-stream shutdown are a new module `tests/integration/shutdown.rs` with a raw-socket SSE reader in `support.rs`; the two harness files are synchronised files that architecture.md does not list, so they are added to `.templatesyncignore` here, and stories 6.1 and 6.4 extend them without touching that file.

**Acceptance Criteria:**

- **Given** `EVENT_QUEUE_URL` unset and set, **When** `Adapters::connect` runs, **Then** `event_source` is `None` or a `SqsEventSource` with `max_messages` 10 and a wait of 5 s on a second SQS client whose operation timeout is `SQS_OPERATION_TIMEOUT + 5 s`, `event_signal` is a `PgEventSignal` on its own lazy pool of one connection, nothing touches the network, and the probe client and the three existing cross-field rules are unchanged
- **Given** `application(adapters, config)`, **When** it is built, **Then** `router()` returns the same router as before, the elector of `Background` is the one whose receiver feeds `ClusterState`, the relay publishes to the hub the routes subscribe to, `ingest` is `Some` only when a source exists and carries `EVENT_MAX_DATA_BYTES`, `prune` carries `EVENT_RETENTION`, `seed` carries `EVENT_TYPES`, the relay carries `EVENT_RELAY_POLL_INTERVAL`, and the schedule is the renew interval, a prune interval of 60 s and a retry delay of 1 s
- **Given** `NODE_ID` set and unset, **When** two requests read `GET /api/cluster`, **Then** `nodeId` is the configured value, or one UUIDv7 generated once and equal in both answers
- **Given** the three `Adapters { .. }` literals, **When** the tests are compiled, **Then** they carry `ScriptedEventSource` or `None` and `ScriptedEventSignal`, and no literal test is deleted or skipped
- **Given** a served binary with one open SSE stream, **When** SIGTERM arrives, **Then** the stream body ends, the process exits with status 0 within `SHUTDOWN_GRACE_PERIOD`, the entries `shutdown signal received` and `stopped` are written and `grace period elapsed, forcing shutdown` is not
- **Given** a served binary that is leader, **When** it stops on SIGTERM, **Then** the row of `cluster_leases` has `expires_at` not after the database `now()` and keeps its holder and term
- **Given** a served binary during shutdown, **When** a new stream is requested after the hub was closed and before the listener stops, **Then** the answer is the 503 problem
- **Given** a served binary with `EVENT_QUEUE_URL` unset, **When** it runs for several relay intervals, **Then** it serves the catalog, a stream and the cluster status, becomes leader, and writes no `ingestion failed` entry
- **Given** the existing serve tests that list the entries written outside requests, **When** they run with the background loops, **Then** the lists are extended with `leadership changed` and with no other new entry on a healthy stack
- **Given** a served binary whose database is unreachable, **When** it starts, **Then** it listens, `GET /api/cluster` answers 200 with `role` `follower` and `leader` `null`, a stream without a cursor is accepted, the catalog answers with the 500 problem, and the entries `lease store failed`, `event signal failed` and `relay failed` are written once per failed round
- **Given** the story's files on top of stories 2.2, 5.1 and 5.2, **When** the session runs `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture` and `make.complexity` and the verify step runs `make.integration_tests`, `make.tests_with_coverage` and `make.e2e_tests`, **Then** all pass with 100 % line and function coverage of `bootstrap.rs` and `serve.rs`, and the tests leave no surviving or timed-out mutant for `make.mutation_tests_diff`

**Depends on:** 2.2, 5.1, 5.2

## Epic 6: Cluster proof, end to end, load and documentation

The epic proves the behaviour of several nodes on one database, adds the Gherkin features of the streams, the K6 scenario and the documents of FR-27, and ends with the full verification list of the repository.

### Story 6.1: Cross-node relay and leader failover integration tests

As a maintainer, I have tests that prove an event ingested by one node reaches the subscribers of another node and that exactly one node leads and another takes over after a release or an expiry. The story adds test modules only; a defect it finds is fixed in the story that owns the code.

**Scope:**

- `crates/service/tests/integration/main.rs` (module lines), `crates/service/tests/integration/support.rs`, `crates/service/tests/integration/cross_node.rs` and `crates/service/tests/integration/failover.rs` (new).

**Covers:** FR-17, FR-18, FR-19, FR-20, FR-21, NFR-5, NFR-6, NFR-7, NFR-8, NFR-12, NFR-19.

> Assumption: the two module names are chosen here; architecture.md names the tests, not their files.

**Acceptance Criteria:**

- **Given** two `Application`s A and B on one `#[sqlx::test]` database, a `ScriptedEventSource` on A, and a subscription for `user.created` on B, **When** A runs `ingest.receive`, `ingest.process` and B runs `relay.tick`, **Then** the subscriber of B receives the event with the sequence the log gave it, and a subscriber of B for another type receives nothing
- **Given** the same two nodes and no listen round on B (a lost notification), **When** B runs its next `relay.tick`, **Then** the event is delivered, so the delay is at most one poll interval and nothing is lost
- **Given** the same two nodes with the real `PgEventSignal`, **When** A appends and B runs `listen.step`, **Then** the wake of B is notified and the next `relay.tick` of B delivers
- **Given** three events ingested by A, **When** subscribers on A and on B read them, **Then** both see the same three sequences in the same order
- **Given** an event ingested before the first relay round of B, **When** B relays and a client subscribes on B, **Then** the event is not delivered live by B and is returned by a subscription with a cursor of 0
- **Given** two electors with `PgLeaseStore` on one database, **When** both campaign several times, **Then** exactly one reports `Leader`, both report the same leader and the term 1, renewals keep the term, and `GET /api/cluster` of each node shows its own `nodeId` and the same `leader`
- **Given** the leader calls `resign`, **When** the other node runs its next campaign, **Then** it is leader with the term 2 and the old leader reads follower with that leader
- **Given** a lease time of 200 ms and a leader that stops campaigning without `resign`, **When** the other node campaigns before and after the expiry, **Then** it stays follower before and becomes leader with the term increased by 1 after
- **Given** a batch that the old leader appended but did not acknowledge, **When** the new leader receives the redelivered batch, **Then** `appended` is 0, no subscriber gets a second copy, and a new message is ingested and delivered by the new leader
- **Given** the story's files on top of story 5.3, **When** the session runs `make.format`, `make.lint_code`, `make.architecture` and `make.complexity` and the verify step runs `make.integration_tests` and `make.tests_with_coverage`, **Then** all pass, every awaiting test is bounded by `tokio::time::timeout`, and coverage stays at 100 % lines and functions

**Depends on:** 5.3

### Story 6.2: Streaming end-to-end features

As a product owner, I read the behaviour of the streams in Gherkin: SSE delivery, WebSocket delivery with run-time commands, resume, and the leader status after an election round. The story keeps one `Application` per scenario in the world, adds the steps that publish through the scripted source and read frames with a deadline, and adds four feature files.

**Scope:**

- Root `Cargo.toml` (`tokio-tungstenite` under the test dependencies), `Cargo.lock`, `tests/e2e/Cargo.toml` (`tokio-tungstenite`, `futures-util` with `sink`, and each workspace crate whose items a step names).
- `tests/e2e/{world.rs,steps.rs}`, `tests/e2e/steps/{event_stream.rs,cluster.rs}`.
- `features/event_stream_sse.feature`, `features/event_stream_websocket.feature`, `features/event_stream_resume.feature`, `features/cluster.feature`.

**Covers:** FR-3 (type of an ingested event), FR-4, FR-5, FR-7, FR-8, FR-9, FR-10, FR-11, FR-13, FR-17, FR-21, NFR-8, NFR-19.

> Assumption: `tests/e2e/Cargo.toml` gets `event-stream-application`, `event-stream-domain` or `cluster-application` only if a step names an item of that crate; architecture.md lists the first and the last, and the unused-dependency gate refuses an entry without a use.

**Acceptance Criteria:**

- **Given** a scenario world that keeps one `Application`, **When** a publish step runs, **Then** it pushes one message into the `ScriptedEventSource` and calls `ingest.receive`, `ingest.process` and `relay.tick`, and a read step takes the next frame of the kept stream with a deadline and fails the scenario when none arrives
- **Given** an SSE subscriber for `user.created`, **When** an event of `user.created` and an event of `order.paid` enter through the source, **Then** the scenario of `event_stream_sse.feature` sees one frame with the fields `id`, `event` `user.created` and `data` equal to the message body, and no frame for `order.paid`
- **Given** the SSE route, **When** the scenarios send `types=User.Created`, 101 types, and `Last-Event-ID: abc`, **Then** the answers are 422 with `/problems/validation-failed`, 422, and 400, each as `application/problem+json`
- **Given** a WebSocket client on the loopback listener of the world, **When** it sends a subscribe command for `user.created` and an event enters, **Then** the scenario of `event_stream_websocket.feature` sees the text frame `{"id":"..","type":"user.created","data":".."}`; after an unsubscribe command a second event produces no frame; an invalid command produces `{"error":"invalid command"}` and the next valid command still works
- **Given** three retained events of a type and a client that received the first, **When** it reconnects with `Last-Event-ID` on the SSE route, and with `lastEventId` on the WebSocket route, **Then** the scenarios of `event_stream_resume.feature` see the second and the third event in order, then a newly published fourth, and no event twice
- **Given** a client that reconnects with a cursor of 0, **When** it reads, **Then** it sees every retained event of its types
- **Given** an event of a type that was never registered, **When** it was ingested, **Then** `GET /api/event-types` lists the type
- **Given** one election round was run through the kept application, **When** `GET /api/cluster` is read, **Then** `cluster.feature` sees `role` `leader`, `leader` equal to `nodeId` and `term` 1
- **Given** the WebSocket scenarios, **When** the world binds its loopback listener, **Then** the binding function carries one item-level `#[expect(clippy::disallowed_methods, reason = "..")]` as the integration support of `crates/service` does, and no other lint expectation is added
- **Given** the story's files on top of stories 4.4 and 5.3, **When** the session runs `make.format`, `make.lint_code`, `make.architecture` and `make.complexity` and the verify step runs `make.e2e_tests`, `make.unused_dependencies` and `make.check_security`, **Then** all pass, every step of the new features is defined, and the five existing features pass unchanged

**Depends on:** 4.4, 5.3

### Story 6.3: K6 event-stream scenario

As a maintainer, I have a K6 scenario that holds 50 SSE and 50 WebSocket subscribers in its smoke profile while events are published to the queue, and checks delivery and its latency inside the budget. The story adds the script and its configuration, publishes the LocalStack port of the load-test stack on loopback and documents the scenario. It depends on stories 2.2 and 5.3 by ownership of the compose files, `docs/environment.md` and `.templatesyncignore`.

**Scope:**

- `tests/Load/scripts/eventStream.js` (new), `tests/Load/config.json.dist` (`endpoints.eventStream` with four profiles, `brokerPort`, `eventQueue`).
- `docker-compose.load_test.override.yml`, `docs/environment.md` (`LOAD_TEST_LOCALSTACK_PORT`), `docs/load-testing.md`, `.templatesyncignore` (the override file and `docs/load-testing.md`).

**Covers:** FR-26, NFR-1 (the profile that measures it), NFR-4.

> Assumption: architecture.md fixes only the smoke values. The entries `average`, `stress` and `spike` follow the shape of the `health` entry; `stress` holds 10 000 subscribers (5 000 virtual users with one SSE and one WebSocket subscriber each) at 100 events per second, so that the NFR-1 target can be measured on the reference machine with `make.load_tests`, outside the pull-request gates. Its `deliveryThreshold` starts at the smoke ceiling of 2000 ms until the first measurement, by the budget rule of `docs/load-testing.md`.
>
> Assumption: when the production image cannot be built in the session, `make.smoke_load_tests` is recorded `not run` with the reason and left to the CI job `Smoke load tests`; the lint of the script still runs.

**Acceptance Criteria:**

- **Given** `tests/Load/config.json.dist`, **When** `endpoints.eventStream.smoke` is read, **Then** it holds `threshold` 500, `rps` 10, `vus` 50, `duration` 20, `hold` 5 and `deliveryThreshold` 2000, the entries `average`, `stress` and `spike` exist with the same keys, the top-level keys `brokerPort` (`14566`) and `eventQueue` (`notification-events`) exist, and the four `health` budgets are unchanged
- **Given** one iteration of the script, **When** it runs, **Then** it gets `GET /api/event-types` and checks status 200, starts one held SSE request on `/api/events/stream?types=load.test` with the timeout `hold` and the tag `test_type` `sse_hold`, opens one `k6/websockets` socket on `/api/events/ws?types=load.test` and checks the handshake, publishes one message to LocalStack with the JSON protocol (`X-Amz-Target: AmazonSQS.SendMessage`, attribute `event-type` `load.test`, tag `test_type` `publish`) whose body holds the virtual user, the iteration and the send time, and closes the socket after `hold`
- **Given** the WebSocket subscriber, **When** the frame with its own marker arrives, **Then** the difference between now and the send time is added to the trend `event_delivery_latency`
- **Given** the held SSE request has ended at its timeout, **When** its partial body is checked, **Then** it holds a frame with `event: load.test` and the marker of this iteration
- **Given** the thresholds of the run, **When** the options are built, **Then** the two thresholds of the protected `ThresholdsBuilder` are used unchanged (the p99 budget on the catalog request and the check rate above 99 %) and the script adds `event_delivery_latency{test_type:<profile>}` with `p(99) < deliveryThreshold`
- **Given** `docker-compose.load_test.override.yml`, **When** the load-test stack is rendered, **Then** `localstack` publishes `127.0.0.1:${LOAD_TEST_LOCALSTACK_PORT:-14566}:4566`, the variable has a row in `docs/environment.md`, and both changed synchronised files are in `.templatesyncignore`
- **Given** `docs/load-testing.md`, **When** the scenario table is read, **Then** it has the row of `eventStream` with its profiles, the meaning of `hold` and `deliveryThreshold` and the note that each held stream logs one `Request Failed` warning
- **Given** the script, **When** the load-script lint target `lint-load-tests` (no profile key) runs, **Then** it passes and the script uses no `console`
- **Given** the production image and the load-test stack, **When** `make.smoke_load_tests` runs, **Then** the `eventStream` smoke profile passes all three thresholds together with the `health` scenario, or the target is recorded as deferred to CI with its reason
- **Given** the story's files on top of stories 2.2, 4.4 and 5.3, **When** `make.docs_check`, `make.validate_configuration`, `make.validate_invariants` and the lint targets `lint-load-tests` and `lint-repo` (no profile key) run, **Then** all pass and `tests/Load/utils/thresholdsBuilder.js`, `tests/Load/eslint.config.js` and the image pins are byte-identical to the base

**Depends on:** 2.2, 4.4, 5.3

### Story 6.4: Served-binary streaming and failover tests

As a maintainer, I have tests that run the real binary against PostgreSQL and LocalStack: a message sent to the queue arrives on an SSE stream and on a WebSocket, an invalid message is logged and removed, a second node takes over after the leader is killed, and SIGTERM closes a WebSocket with code 1001. The story adds a test module and the WebSocket client dependency of the service tests. It depends on story 6.2 by ownership of the root `Cargo.toml` and `Cargo.lock`, and on story 6.1 by ownership of the integration harness.

**Scope:**

- `crates/service/Cargo.toml` (dev `tokio-tungstenite`, dev `futures-util` with `sink`), `Cargo.lock`.
- `crates/service/tests/integration/main.rs` (module line), `crates/service/tests/integration/support.rs`, `crates/service/tests/integration/streaming.rs` (new).

**Covers:** FR-4, FR-7, FR-15, FR-16, FR-17, FR-18, FR-20, FR-21, FR-24, NFR-5, NFR-6, NFR-20.

> Assumption: the takeover test asserts the bound `CLUSTER_LEASE_TTL + CLUSTER_RENEW_INTERVAL` plus a fixed scheduling margin of the test, with a lease time of 1 s and a renew interval of 200 ms, because a real-time bound without a margin would be flaky on a shared runner.
>
> Assumption: the module name `streaming.rs` is chosen here.

**Acceptance Criteria:**

- **Given** a served binary with `EVENT_QUEUE_URL` set to a queue created by the test and an SSE stream for `user.created` read from a raw socket, **When** a message with the attribute `event-type` `user.created` is sent to the queue, **Then** one frame arrives with a decimal `id`, `event: user.created` and the body as `data`, and `GET /api/event-types` lists `user.created`
- **Given** the same binary, **When** the same `event-id` is sent twice, **Then** exactly one frame arrives and both messages leave the queue
- **Given** the same binary, **When** a message without `event-type` is sent and then a valid one, **Then** the process log holds one `inbound message rejected` entry with the message id and the reason and without the body, the invalid message leaves the queue, and the valid one is delivered
- **Given** the same binary and a `tokio-tungstenite` client on `/api/events/ws` without `types`, **When** the client sends a subscribe command and a message is sent to the queue, **Then** a text frame with the members `id`, `type` and `data` arrives, and with `STREAM_KEEP_ALIVE=1s` a ping arrives within 2 s
- **Given** a stream that stays open longer than `HTTP_REQUEST_TIMEOUT`, **When** a message is sent after that time, **Then** it is still delivered
- **Given** an open WebSocket and an open SSE stream, **When** the binary gets SIGTERM, **Then** the WebSocket receives the close code 1001, the SSE body ends, and the exit status is 0 within the grace period
- **Given** two served binaries on one database and one queue, **When** both have campaigned, **Then** exactly one `GET /api/cluster` answers `role` `leader`, both name the same `leader` and `term`, and a message sent to the queue reaches an SSE client of the follower
- **Given** the two binaries with a lease time of 1 s and a renew interval of 200 ms, **When** the leader is killed without a signal it can handle, **Then** the other node reports `leader` with the term increased by 1 within the bound of the assumption above, and a message sent afterwards is delivered to its client
- **Given** the story's files on top of stories 4.4, 6.1 and 6.2, **When** the session runs `make.format`, `make.lint_code`, `make.architecture` and `make.complexity` and the verify step runs `make.integration_tests`, `make.tests_with_coverage`, `make.unused_dependencies` and `make.check_security`, **Then** all pass, every waiting read is bounded by a timeout, and coverage stays at 100 % lines and functions

**Depends on:** 4.4, 6.1, 6.2

### Story 6.5: Protocol document, decision records and operations notes

As a client developer and as an operator, I read the contract of the two streaming routes, the delivery guarantees, the reasons for the main decisions, and how the cluster behaves in operation. The story writes `docs/streaming-protocol.md`, the decision records 0014 to 0017 and the updates of the request collection, the operations guide, the security notes, the README and the architecture model descriptions. It depends on story 6.3 by ownership of `.templatesyncignore`.

**Scope:**

- `docs/streaming-protocol.md` (new), `docs/adr/0014-event-log-and-listen-notify.md`, `docs/adr/0015-event-source-port-sqs-first.md`, `docs/adr/0016-streaming-routes-outside-openapi.md`, `docs/adr/0017-opaque-text-event-data.md`, `docs/adr/README.md`.
- `docs/requests.http`, `docs/operations.md`, `docs/architecture.md`, `docs/README.md`, `SECURITY.md`, `README.md`.
- `workspace.dsl` (descriptions only), `.templatesyncignore` (`docs/operations.md`, `docs/README.md`, `SECURITY.md`).

**Covers:** FR-27, NFR-13, NFR-15 (log schema in the operations guide), NFR-11 and NFR-7 (documented behaviour).

**Acceptance Criteria:**

- **Given** `docs/streaming-protocol.md`, **When** it is read, **Then** it specifies for both routes the parameter `types` with its limits (100 names, 128 characters, the pattern), the two cursor carriers and that the header wins, the answers before a stream opens (400, 422, 503, 500 and the 426 of a request that is not an upgrade), the SSE frame with `id`, `event` and `data`, the line-break rule (`\r\n` and `\r` arrive as `\n`), the keep-alive comment, the WebSocket server frame with `id` as a string, the two commands, the error frame with its six reasons, the ping, the close codes 1013, 1001 and 1011, and the size limit of a command
- **Given** the same document, **When** its guarantees are read, **Then** it states the seven delivery guarantees of the PRD, that sequences have gaps, that a cursor older than `EVENT_RETENTION` loses events without a signal, the reconnect procedure, the SQS envelope with its three attributes, and that the routes are not in the OpenAPI document and why
- **Given** `docs/requests.http`, **When** it is read, **Then** it holds one example for `GET /api/events/stream` and one for `GET /api/events/ws`
- **Given** `docs/operations.md`, **When** it is read, **Then** it describes the leader lease and its term, the takeover bounds (13 s after a crash and 3 s after a graceful stop with the defaults), retention and pruning, the extra listener connection per node (`DATABASE_MAX_CONNECTIONS + 1`), that `LISTEN` does not pass a transaction-pooling proxy and the poll is the fallback, the advisory-lock key, the behaviour during a database outage and a queue outage, the shutdown order, and the table of the log entries with their levels and fields
- **Given** `SECURITY.md` and the protocol document, **When** they are read, **Then** both say that there is no subscriber authorization yet, that the service must carry only events that are safe for every client and must not be exposed with private data on the bus, and name the follow-up (JWT from the user-service OAuth server, private topics)
- **Given** the decision records, **When** `docs/adr/README.md` is read, **Then** the records 0014 to 0017 are indexed and each has the form context, decision, consequences with the decision and the main reason of architecture.md
- **Given** `README.md`, `docs/README.md` and `docs/architecture.md`, **When** they are read, **Then** the README describes the three-step frontend flow, the index lists the protocol document, and the operational assumptions say that a node now holds per-connection state and background loops and what is out of scope (authorization, Kafka adapter, schema repository)
- **Given** `workspace.dsl`, **When** the descriptions are updated (the container also serves SSE and WebSocket event streams, the relationship of `apiClient` names the streams, the message broker is also the event bus), **Then** `make.validate_architecture_model` passes
- **Given** the three changed synchronised files, **When** the story ends, **Then** `docs/operations.md`, `docs/README.md` and `SECURITY.md` are in `.templatesyncignore`
- **Given** the story's files on top of stories 4.4, 5.3 and 6.3, **When** `make.docs_check`, the repository lint target `lint-repo` (no profile key), `make.validate_invariants` and `make.validate_architecture_model` run, **Then** all pass: every cited target, command, path and route exists, and no Rust file changed

**Depends on:** 4.4, 5.3, 6.3

### Story 6.6: Full verification of the change

As the maintainer who merges the change, I get the verification list of `AGENTS.md` run in its order on the final head, with every target recorded and the mutation gate run on the whole change. The story changes no file unless a gate fails; a failure is fixed at its root in the code of the owning story's files and the list is run again from the failed target.

**Scope:**

- No file of its own. A fix touches only the files of the story whose code fails, never a protected file.

**Covers:** NFR-4, NFR-14, NFR-16, NFR-17, NFR-18, NFR-19, NFR-20, and the gate evidence of every FR.

> Assumption: the four targets that need the production image (`build-prod-image`, `make.image_scan`, `make.schemathesis_validate`, `make.smoke_load_tests`) are recorded as deferred to CI with the reason when the image cannot be built in the session; the story then ends with a report, not with a pass claim for them. The NFR-1 measurement on the reference machine is outside this list and is recorded as not run.

**Acceptance Criteria:**

- **Given** every story from 1.1 to 6.5 is committed on the feature branch, **When** `make.validate_configuration` (with `base_ref=<base>` and an empty `CONFIG_CHANGE_APPROVED=`), `make.validate_invariants`, `make.secrets_scan`, the lint targets `lint-repo` and `lint-load-tests` (no profile key) and `make.validate_architecture_model` run in this order, **Then** each passes: no item of `protected-config.toml` differs from the base, no secret is found, and the model has one component per workspace crate and one relationship per workspace-internal dependency
- **Given** the preflight targets passed, **When** `make.format_check`, the target `validate-manifest` (no profile key), `make.check_crate_isolation`, `make.static_analysis`, `make.complexity`, `make.architecture`, `make.check_security`, `make.unused_dependencies`, the target `check-requirements` (no profile key) and `make.docs_check` run in this order, **Then** each reports zero findings, the diff against the base holds no `#[allow`, no `#[mutants::skip]`, no `#[coverage(off)]` and no new exclusion, and no `event-stream` crate depends on a `cluster` crate or the reverse
- **Given** the static targets passed, **When** `make.setup_test_db`, `make.validate_sqlx_metadata`, `make.tests_with_coverage`, the target `doc-tests` (no profile key) and `make.e2e_tests` run in this order, one DB-backed target at a time, **Then** the twelve new files of `.sqlx/` are current, coverage is 100 % of lines and 100 % of functions with no exclusion added, and every feature passes
- **Given** the test targets passed, **When** `make.validate_contract_snapshots`, `make.validate_openapi_spec`, `make.openapi_diff` and `make.graphql_diff` run in this order with `base_ref=<base>`, **Then** the snapshots equal the generated contracts, Spectral reports nothing, and the diffs report additions only: the paths `/api/event-types` and `/api/cluster`, the schemas `EventType`, `ClusterStatus` and `ClusterRole`, two tags, the GraphQL type `EventType` and the query field `eventTypes`
- **Given** the whole change is committed, **When** `make.mutation_tests_diff` runs against the base ref, **Then** it reports zero surviving and zero timed-out mutants in the changed code
- **Given** the container engine, **When** the target `build-prod-image` (no profile key), `make.image_scan`, `make.schemathesis_validate` and `make.smoke_load_tests` run in this order, **Then** the image builds, the scan finds no HIGH or CRITICAL vulnerability, Schemathesis passes for the documented operations including the two new ones, and the smoke profiles stay inside their budgets; or, when the image cannot be built here, each of the four is recorded `not run` with that reason and named as deferred to CI
- **Given** the delivery targets are recorded, **When** the target `bats` (no profile key; the stage key is `make.ci_cli`) runs, **Then** the Makefile tests pass and no file under `tests/CLI/bats/` changed
- **Given** all 31 targets of the list are recorded, **When** the close-out is written, **Then** it names every target with `passed`, `failed` or `not run (<reason>)` in the order of `AGENTS.md`, the gates that prove the change (`make.tests_with_coverage`, `make.mutation_tests_diff`, `make.static_analysis`, `make.complexity`, `make.architecture`, `make.validate_contract_snapshots`, `make.docs_check`, `make.validate_architecture_model`, `make.validate_configuration`), every deviation from a skill or a rule, and no target is reported as passed that did not run

**Depends on:** 6.1, 6.2, 6.3, 6.4, 6.5

**Requirement coverage:**

| Requirement | Stories |
| --- | --- |
| FR-1 | 1.1 |
| FR-2 | 1.1 |
| FR-3 | 1.1, 2.1, 3.3, 3.5, 5.2, 6.2 |
| FR-4 | 4.3, 6.2, 6.4 |
| FR-5 | 3.1, 3.2, 4.3, 6.2 |
| FR-6 | 4.3 |
| FR-7 | 4.4, 6.2, 6.4 |
| FR-8 | 3.2, 4.4, 6.2 |
| FR-9 | 4.2, 4.4, 6.2 |
| FR-10 | 4.4, 6.2 |
| FR-11 | 3.1, 3.4, 4.2, 4.3, 4.4, 6.2 |
| FR-12 | 3.3, 3.4, 5.2 |
| FR-13 | 4.3, 4.4, 6.2 |
| FR-14 | 3.1, 3.5, 5.1 |
| FR-15 | 2.2, 3.5, 5.1, 5.3, 6.4 |
| FR-16 | 3.5, 6.4 |
| FR-17 | 3.4, 3.5, 5.2, 6.1, 6.2, 6.4 |
| FR-18 | 3.2, 3.4, 5.2, 5.3, 6.1, 6.4 |
| FR-19 | 1.2, 5.2, 6.1 |
| FR-20 | 1.2, 5.2, 5.3, 6.1, 6.4 |
| FR-21 | 1.2, 5.3, 6.1, 6.2, 6.4 |
| FR-22 | 4.1, 4.2, 4.3, 4.4 |
| FR-23 | 4.1, 4.2, 4.3, 4.4 |
| FR-24 | 4.1, 4.4, 5.2, 5.3, 6.4 |
| FR-25 | 1.2, 2.1, 2.2, 5.3 |
| FR-26 | 6.3 |
| FR-27 | 6.5 |
| NFR-1 | 4.1, 6.3 |
| NFR-2 | 3.2, 4.1 |
| NFR-3 | 3.2, 4.2 |
| NFR-4 | 6.3, 6.6 |
| NFR-5 | 5.3, 6.1, 6.4 |
| NFR-6 | 1.2, 5.2, 5.3, 6.1, 6.4 |
| NFR-7 | 1.2, 6.1, 6.5 |
| NFR-8 | 3.1, 3.4, 3.5, 4.2, 6.1, 6.2 |
| NFR-9 | 4.1, 4.2 |
| NFR-10 | 4.1, 5.2 |
| NFR-11 | 4.2, 4.3, 5.1, 5.2, 5.3, 6.5 |
| NFR-12 | 3.4, 5.3, 6.1 |
| NFR-13 | 6.5 |
| NFR-14 | 2.1, 3.1, 3.4, 3.5, 4.3, 4.4, 5.1, 6.6 |
| NFR-15 | 1.2, 3.5, 4.2, 4.3, 4.4, 5.2, 5.3, 6.5 |
| NFR-16 | every story (own tests), 6.6 |
| NFR-17 | every story (own lint and complexity runs), 6.6 |
| NFR-18 | 1.1, 1.2, 3.2, 3.4, 6.6 |
| NFR-19 | 3.1, 3.2, 4.4, 6.1, 6.2, 6.6 |
| NFR-20 | 1.1, 1.2, 4.3, 6.4, 6.6 |
