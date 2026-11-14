# Research: real-time event delivery hub (issue #4)

Bundle `specs/4-realtime-event-delivery-hub/`, issue #4 as updated on 2026-11-14, branch `claude/vilnacrm-notification-service-br7s47`, head `ecc5d17` (base `origin/main` = `3f28127`). This file replaces the research of the first plan (PostgreSQL event log, leader lease, SQS source) completely.

It condenses three fact files written for this re-plan on 2026-11-14 and the evidence list of the design input:

- `replan/repo-facts.md`: the repository as it is.
- `replan/kafka-facts.md`: reading Kafka with `krafka =0.32.0`, probed against `apache/kafka:4.1.1`.
- `replan/postgres-removal.md`: everything the removal of PostgreSQL and of the example context touches.
- `DESIGN-INPUT-V2.md`: the owner's decision and the binding decisions D1 to D18.

All four live in the session scratchpad, not in the repository. No fact file was missing.

Tags used below:

- **[V]** verified on 2026-11-14, by reading the file and line named, by command output, or by a probe.
- **[V probe]** run against a real `apache/kafka:4.1.1` broker (KRaft, one broker). The probe code and its output are in `replan/probes/`.
- **[U]** not verified: from memory, inferred, or not run.

`K/` is the source of `krafka-0.32.0` in the cargo registry. Paths without a prefix are relative to the repository root.

## 1. Decision and scope

- The owner decided on 2026-11-14: "Kafka + rust nodes that all receive all events and filter it by themselves. Deduplication works on the client side." He also said: "Remove postgresql since we will not use it too."
- Every node reads the one notifications topic itself, without a consumer group. There is no leader, no election, no lease, no lock and no link between nodes (D1). The topic has exactly one partition (D2). The wire id of an event is its offset + 1 (D3).
- PostgreSQL leaves the service and the repository, and so does the PostgreSQL-backed example context `company-subdomain/some-module` (D10). SQS and LocalStack stay as the template ships them; the notification code does not use them (D11).
- One new bounded context: `notification/event-stream` (D12). The first plan's `internal/cluster` and `GET /api/cluster` are gone.

## 2. Current state of the touched contexts

### 2.1 Workspace and layers

- The workspace has 13 members [V `Cargo.toml:3-17`]:
  - three contexts times three layers: `shared`, `internal/health-check`, `company-subdomain/some-module`;
  - `crates/service`, `tests/e2e` (`e2e-tests`), `tests/support` (`test-support`) and `xtask`.
- No notification crate exists. The story 1 rule therefore applies: story 1.1 scaffolds `notification/event-stream` [V `rs-sdlc-plan.md` step 5].
- `Cargo.lock` holds no `krafka`, no `rdkafka` and no `tungstenite`. It already holds `socket2 0.6.5`, `tokio-util 0.7.20`, `bytes 1.12.1` and `serde_json 1.0.154` [V].
- The layer allow-lists in `architecture.toml:11-89` (protected) [V]:

| Layer | `external` today | Relevant gaps |
| --- | --- | --- |
| domain | `async-trait`, `thiserror`, `uuid` | `tokio` is forbidden even transitively |
| application | `async-graphql`, `async-trait`, `axum`, `futures-util`, `humantime`, `serde`, `thiserror`, `tokio`, `tracing`, `utoipa`, `utoipa-axum`, `uuid`, `validator` | `serde_json` only in `dev_external`; no `tokio-tungstenite` |
| infrastructure | `async-trait`, `aws-config`, `aws-sdk-sqs`, `moka`, `sqlx`, `thiserror`, `tracing`, `uuid` | no `krafka`; `tokio` only in `dev_external`; `async-graphql`, `axum`, `utoipa` forbidden transitively |
| test-support | `async-trait`, `fake`, `rand`, `tracing`, `uuid` | no `tokio`, no `krafka` |
| service, e2e-tests, xtask | `*` | none |

- How the gate works [V `xtask/src/arch/rules.rs`, `load.rs`]:
  - `external` judges every direct dependency of a manifest by name.
  - `forbidden_transitive` judges what is reachable through non-dev edges.
  - A context may depend on itself and on `shared` only, so `event-stream` and `health-check` may not depend on each other.
  - A crate sits at `crates/[<group>/]<context>/<layer>`.
- `make new-context group=<g> context=<c>` writes three manifests and three `lib.rs` files with no dependency and refreshes the lock offline [V `xtask/src/scaffold.rs:71-110`]. It generates no sqlx code. Only one template sentence says "SQL lives here" (`xtask/templates/infrastructure/lib.rs.tmpl:5`, not protected).
- The plugin's scaffolding checklist has twelve items, C1 to C12 [V plugin `skills/bounded-context-scaffolding/SKILL.md:102-115`]:
  - C8 (migration, `.sqlx`) does not apply: the context stores nothing.
  - The skill's success criterion "no protected file changed" (`:45`) holds for the scaffold itself. The allow-list entries come in the preparatory pull request of story 3.1.

### 2.2 House rules that shape the code [V]

- Errors are RFC 9457 problems (`crates/shared/application/src/http/problem.rs:45-141`). A 5xx is produced only through `Problem::internal`.
- Handlers may use only the extractors `ApiPath`, `ValidatedQuery`, `ValidatedJson` and `ApiJson` (`crates/shared/application/src/http/extract.rs`).
  - A test fails when a `pub async fn` in `crates/service/src/http` or `src/graphql` uses another parameter type, or when the set of files holding such functions changes (`crates/service/src/http/router/tests.rs:423-520`).
- Disallowed methods (`clippy.toml:30-50`): `SystemTime::now`, `Instant::now`, `Uuid::now_v7`, `new_v4`, `TcpListener::bind`, `TcpStream::connect` and `std::env::var*`. Time goes through the `Clock` port and ids through the `UuidGenerator` port.
- No `#[allow]`; an `#[expect]` carries a reason. No `mod.rs`. Unit tests live in a sibling `tests.rs`.
- Complexity limits: cognitive complexity 8, 50 lines, 5 arguments and nesting 5, for `--lib --bins`.
- Lints that shape the code: `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `arithmetic_side_effects`, `as_conversions`, `wildcard_enum_match_arm`, `missing_docs` and `unsafe_code = "forbid"`.
- No logic in an `#[async_trait]` method body (`docs/adr/0008`).
- Tools run only through make; the git hooks are never bypassed.

### 2.3 Configuration [V]

- Every setting is an `Option<String>` clap argument with `hide_env_values = true` (`crates/service/src/cli.rs`).
- `Config::from_args` collects every problem. Cross-field rules come last, and `cross_field_rules` already has five parameters, the limit (`config.rs:93-124`, `:202-262`).
- Problems are reported as `<VARIABLE>: <reason>`, with exit status 2. A reason never holds a value.
- Two variables are required today: `DATABASE_URL` and `AWS_REGION` (`config.rs:278`, `:300`). `serve` reads 19 variables.
- `docs/environment.md` must list exactly the union of the variables that `make docs-check` rule 5 collects: `[env: X]` in the help, the four compose files, `.env`, `.env.test`, `?=` make variables and `.cargo/config.toml` `[env]` (`scripts/check-docs.sh:447-490`). A fifth compose file is not read.
- Value forms in `.env` are limited by the protected `scripts/with-env.sh`: no unquoted white space, no `${X:-y}`.

### 2.4 Composition root and runtime [V]

- `Adapters { repository, probes, event_bus, clock, uuid_generator }` is built by `Adapters::connect(&Config)` without network access. A test pins that and the probe order: database, cache, message broker (`crates/service/src/bootstrap.rs:36-87`, `tests/integration/bootstrap.rs:83-108`).
- Middleware runs outermost first: request id, security headers, `TraceLayer`, `CatchPanicLayer`, request budget, body limit (`src/http/router.rs:34-47`). The request budget wraps only `next.run(request)`, so a long-lived stream is bounded only up to its response head.
- No background task exists today. `serve` binds, logs `listening`, serves with graceful shutdown and waits `SHUTDOWN_GRACE_PERIOD`. Exit statuses: 0 drained, 1 forced or failed, 2 invalid configuration (`command/serve.rs:104-167`).
- `axum` is built with `macros` only. The `ws` feature would add `tokio-tungstenite`, `sha1` and `base64`. `axum::response::sse` exists. `tower-http` `cors` adds no crate. `OpenApiRouter::route` adds an undocumented route. `axum::serve::ListenerExt::tap_io` and `socket2::SockRef::set_tcp_user_timeout` exist [V crate sources].
- `WebSocketUpgrade` rejections are plain text, so they must be converted to problems (`axum-0.8.9/src/extract/ws.rs:951-987`).

### 2.5 PostgreSQL today [V]

- `DATABASE_URL` is required by `serve`, `check-config` and `migrate`.
- `Adapters::connect` builds a lazy pool for the example repository and the `PostgresProbe` (`SELECT 1`).
- The example context runs three `sqlx::query!` statements, with three files in `.sqlx/` and one migration pair in `migrations/`.
- Compose has `database` and `migrate` services.
- Every test target depends on `setup-test-db` (the protected `scripts/setup-test-db.sh`).
- CI installs `sqlx-cli` and runs `validate-sqlx-metadata` (gate G30, job `query-metadata`).

### 2.6 SQS today [V]

- `sqs_client`, `SqsProbe`, the `AWS_*` and `SQS_*` settings, and the `localstack` service in compose. The new context ignores them.
- `AWS_REGION` stays required, so a node needs it set. `/api/health` keeps the SQS probe.

### 2.7 Local stack, CI and tests [V]

- There are four compose files: `docker-compose.yml`, `.override.yml`, `.prod.yml` and `.load_test.override.yml`.
- Dependencies are started by make, never by workflow `services:`. `ensure-deps` runs `docker compose up --detach --wait --no-recreate $(DEPS)` with `DEPS ?= database localstack` (`Makefile:48`, `:140-148`).
- The `DEPS` text is pinned by protected Bats tests (`make_execution_mode.bats:136`, `make_general.bats:583`). A service-owned target in `make/service.mk` can start the broker without touching `DEPS`; a dry run showed it [V].
- Private projects in the Bats lifecycle tests get free host ports for `DB_PORT`, `LOCALSTACK_PORT`, `APP_PORT` and `STRUCTURIZR_PORT` only. A fixed Kafka host port in a file that `make start` reads would collide [U, derived from the files].
- The isolated load-test stack starts every service of `docker-compose.yml`, so a `kafka` service joins it with no change to the protected script (`scripts/with-load-test-stack.sh:33-52`).
- A protected test runs the production image with only `DATABASE_URL`, `AWS_REGION` and the SQS variables and waits for `healthy` (`make_security.bats:333-356`). `serve` must therefore start with no Kafka variable set.
- The idle spawned-binary tests compare the whole log, `["listening", "shutdown signal received", "stopped"]`, within 2 s (`tests/integration/serve.rs:128-148`).
- Coverage is measured in one run over `--lib --bins --test integration` with a gate of 100 % lines and functions and no exclusion. Mutation testing excludes only `main.rs`, `build.rs` and tests; a timeout counts as surviving (`make/gates.mk:20-28`, `.cargo/mutants.toml`).
- nextest uses a 30 s slow timeout with terminate-after 2. nextest runs tests in parallel processes on one broker, so each test needs its own topic.
- Durations recorded by the template before any notification code: `make ci` took 24 min 42 s locally on 4 vCPU [V `docs/operations.md`]. This is the only timing in this file. It is not a figure for this design.

### 2.8 Load test [V]

- The runner executes `tests/Load/scripts/*.js` in byte order. A protected test expects `health` to fail first, so a new scenario must sort after it: `subscriptions` does (`make_load_tests.bats:288-303`).
- K6 is pinned to `grafana/k6:2.3.0`. It has `k6/http`, `k6/ws`, `k6/websockets` and `k6/timers`, but no Kafka module. Importing `k6/x/kafka` makes it try a download (exit 107 without network) [V probe].
- Only the four `health` thresholds are protected (`protected-config.toml:69-76`).
- The cheapest publisher [V probe]:
  - a shell loop in the load-test compose override that runs the Kafka image's `kafka-console-producer.sh`;
  - with `linger.ms=0`, records were written 1 ms after their send time in steady state;
  - the first records took 1.3 to 1.5 s (JVM start), so the first seconds are discarded.

### 2.9 Contracts and documentation [V]

- `make validate-contract-snapshots` regenerates both snapshots and requires a clean `git status`. Spectral, oasdiff, the GraphQL diff and Schemathesis follow.
- Pinning tests [V `crates/service/src/openapi/tests.rs`, `src/graphql/schema/tests.rs`]:
  - the OpenAPI document has four operations and five schemas;
  - the SDL must not contain `node`, `Connection`, `update`, `delete` or `type Subscription`, descriptions included.
- `docs/streaming-protocol.md` does not exist.
- `workspace.dsl` has one component per crate. Its message-broker element is SQS.
- Synchronised files that this change edits must be listed in `.templatesyncignore` by full path (`make_repository.bats:183-430`).

## 3. Reading Kafka with `krafka =0.32.0`

### 3.1 API used to read one partition without a group [V source]

- `Kafka::builder(bootstrap)` takes `.client_id`, `.security(AuthConfig)`, `.request_timeout`, `.connect_timeout`, and then `connect().await` (`K/src/client.rs`).
  - Clones of a `Kafka` handle share one connection pool.
- `Kafka::consumer_without_group()` returns a `ConsumerBuilder` with `isolation_level`, `auto_offset_reset`, `fetch_max_wait`, `max_poll_records`, `max_partition_fetch_bytes` and `metrics_push`. `build()` contacts no broker.
- `Consumer` offers:
  - `assign`;
  - `seek`, which is local only;
  - `poll(timeout)`, which is cancel safe;
  - `position`;
  - `fetch_watermarks`, which sends two ListOffsets requests;
  - `fetch_metadata`, `wakeup` and `close`.
- `ConsumerRecord` carries `offset`, `value`, `key`, `headers` and `timestamp`. It is `#[non_exhaustive]`.
- Production uses `kafka.producer().acks(All).linger(0).metrics_push(false)` and `send(Record::new(topic, value).partition(0))`. It is used by tests only.
- [V probe 02] Six records were produced and read back after `assign` + `seek(0)` + `poll`, in 19 ms. A poll at the end returned `Ok(empty)` after its timeout.

### 3.2 Behaviour the service must handle

| Fact | Tag | Consequence for the design |
| --- | --- | --- |
| Default isolation is read-uncommitted and delivers aborted records | [V probe 06] | set read-committed explicitly |
| Under read-committed the end is the last stable offset; transaction markers and aborted records take offsets, so offsets are not dense | [V probe 06] | "caught up" compares `position()` with the end, never last offset + 1; clients see gaps |
| Default `auto_offset_reset` is `Latest`: a position below the log start jumps silently to the end | [V probe 04] | use `None`; `poll` then returns `NoOffset`, handled from the watermarks |
| A seek above the end gives no error on a real broker: the client logs a warning and rewinds to the end | [V probe 04] | compare a cursor with the watermarks in the service |
| A missing topic: `fetch_metadata` returns `Ok` with no topic, `assign` and `poll` succeed, and about 10 warnings per second are logged; the client never auto-creates | [V probe 05] | check existence and partition count at start, explicitly |
| `poll` never reports an outage; it returns `Ok(empty)` when the broker is stopped, killed or paused | [V probes 08-11] | detect outages from `fetch_watermarks`, which returns the error |
| After an unclean outage the client's per-partition backoff (up to 30 s, not configurable) delayed new records by 20.45 s on a plain tail; a re-seek to the own position brought them in 0.12 to 0.48 s | [V probe 11a-c] | re-seek after a failed round, and when the watermarks show the end above the position |
| `poll(timeout)` is bounded by `request_timeout` (default 30 s), not by its argument, against a hung broker | [V probe 09] | set both client time-outs and wrap each call in the service's own time-out |
| `request_timeout >= connect_timeout` is enforced | [V probe 07] | `KAFKA_CONNECT_TIMEOUT` may not exceed `KAFKA_REQUEST_TIMEOUT` (cross-field rule) |
| `connect()` makes one attempt; a refused port fails in under 5 ms | [V probe 07] | the service retries; an address that never answers was not probed [U] |
| One connection per handle: a parked long poll delayed a ListOffsets on the same handle by 4.70 s; on two handles it took 2.5 ms | [V probe 12] | separate handles for the tail, the watermark reader and each replay permit |
| Fetch size limits are soft; a 2 MiB record came through with a 1 KiB fetch limit | [V probe 14] | `EVENT_MAX_DATA_BYTES` is checked by the service; one batch may exceed a page budget |
| All five compression codecs decode with the default features | [V probe 13] | no feature for decoding |
| An undecodable batch makes `poll` return `Err(Protocol)` until the application seeks past it | [V source, not probed] | the service does not skip it: `Unavailable`, the round fails, readiness fails by staleness (design brief, open point 1) |
| A consumer dropped without `close()` logs a warning | [V source] | the `krafka=error` log filter of the architecture |

### 3.3 Security, licence and versions

- TLS goes through rustls 0.23 with the roots of `webpki-roots` [V source]. The default crypto provider is `ring`; the feature `rustls-aws-lc-rs` selects aws-lc-rs.
- SASL PLAIN, SCRAM and OAUTHBEARER are always compiled [V source].
- `AuthConfig::plaintext`, `ssl`, `sasl_plain`, `sasl_scram_sha256`, `sasl_scram_sha512` and `with_tls` exist [V source, read by the architecture author in `K/src/auth/mod.rs`]. None of them was probed against a broker [U].
- Licence: `MIT OR Apache-2.0` [V]. No `openssl-sys` and no `native-tls` among its direct dependencies [V].
- `ring` and `aws-lc-rs` are both already in `Cargo.lock` [V, architecture author].
- `cargo tree` and `cargo deny check` were not run with `krafka` [U]. Whether `aws-lc-rs` stays in the lock after `sqlx` leaves was not checked [U].
- Release history: 0.32.0 is dated 2026-11-14 [V `K/CHANGELOG.md`].
  - 16 releases in 14 weeks, with breaking changes in minor releases and one author.
  - `rust-version` 1.95, edition 2024.
  - Broker floor Kafka 3.9; probed against 4.1.1 only.
- Whether `wildcard_enum_match_arm` fires on matches over the `#[non_exhaustive]` `KrafkaError` was not checked [U].

### 3.4 Test support [V source, never run]

- With the feature `test-broker`, `krafka::testing::FakeBroker` offers:
  - an in-memory or TCP broker;
  - `crash` and `restart`;
  - request hooks with `Delay`, `Disconnect`, `Silence` and `CorruptRecords`;
  - `create_topic`;
  - state access that can move the log start;
  - real Produce with transactions.
- It does not serve OffsetForLeaderEpoch, so a seek above the end becomes `NoOffset` where a real broker clamps. Its Fetch handler does not check the log start. It has no TLS and SASL PLAIN only.
- No probe ran against the fake broker [U].

### 3.5 Broker image [V]

- The image is `apache/kafka:4.1.1`. Its local RepoDigest is `sha256:0bc1bb2478f45b6cea78864df86acdc11e8df2c5172477819a4d12942cbe5d40`. The digest was read from the local image, not from the registry [U for the registry].
- The image has no `HEALTHCHECK`, has `nc` and `wget` but no `curl`, and runs as user `appuser`.
- Every `KAFKA_X_Y` variable becomes `x.y`. When variables are given, the generated `server.properties` holds only them.
- The KRaft single-node settings that worked are listed in `kafka-facts.md` section 10. The client connects to the advertised address after bootstrap.
- Topic creation with `kafka-topics.sh --create --partitions 1` worked. `--if-not-exists` was not probed [U].
- Start times seen: about 5 s after a clean stop and about 15 s after a kill.
- Production settings (replication factor 3, `min.insync.replicas=2`, acks=all) and failover timing were not looked up [U].

## 4. What the removal of PostgreSQL touches

Read the inventory itself for line numbers (`postgres-removal.md`). Its findings that shape the order of work [V]:

1. **Required directories.** The configuration guard reads `required_directories` from the base branch, and nothing waives a missing directory, not even the label (`xtask/src/config_guard.rs:77-98`). `.sqlx/` and `migrations/` can therefore be deleted only after `main` holds the shortened list. A preparatory pull request goes first.
2. **Second label.** Removing the example routes and GraphQL fields is a breaking contract change: `make openapi-diff` and `make graphql-diff` need `approved-breaking-change`. That oasdiff reports a removed path as ERR is [U].
3. **13 crates.** A protected test asserts at least 13 workspace crates (`xtask/tests/integration/arch.rs:35`, `:293`). `notification/event-stream` is scaffolded before the three example crates go.
4. **GraphQL never empty.** The schema merges only the example roots today. The protected G20 test removes the first single-line field of the first sorted type (`make_contracts.bats:83-91`, `:209-227`). `eventTypes` must be wired first. The first sorted type (`EventStreamStatus`) needs several single-line fields.
5. **`service migrate`.** Removing it changes the protected `scripts/check-docs.sh:81-82`, `:453` and `check_docs.bats:558`.
6. **Hash pin.** `features/health_check.feature` is pinned by hash in `make_general.bats:81-94`, and its database scenario goes.
7. **Bats tests.** Many protected Bats tests encode the database, so the whole removal is approval-gated.
8. **Template sync.** Template sync restores deleted or changed synchronised files unless they are listed in `.templatesyncignore`.

Scope of the removal [V]:

- **Crates:** `crates/company-subdomain/` (3 crates); the persistence module of `shared/infrastructure`; the PostgreSQL probe and `Dependency::Database`; `migrate`, `DatabaseArgs`, `PostgresSettings` and the rule `HEALTH_PROBE_TIMEOUT >= DATABASE_ACQUIRE_TIMEOUT` in `crates/service`; the `sqlx` and `some-module-*` workspace dependencies; `fake` when unused.
- **Variables:** `DATABASE_URL`, `DATABASE_MAX_CONNECTIONS`, `DATABASE_ACQUIRE_TIMEOUT`, `DATABASE_STATEMENT_TIMEOUT`, `DATABASE_WAIT_TIMEOUT`, `DB_*`, `SQLX_OFFLINE`.
- **Make targets:** `migrate`, `migrate-rollback`, `create-migration`, `setup-test-db`, `setup-load-test-db`, `sqlx-prepare`, `validate-sqlx-metadata`, `ensure-database`; the `setup-test-db` prerequisite of the test targets; `EXEC_ENV`, `TEST_ENV`, `DEPS` and `CI_TESTS_TARGETS`.
- **Stack and CI:** the `database` and `migrate` services and `db_data` in compose; `.env`, `.env.test`, the `Dockerfile` and the dev container; the `query-metadata` job and `sqlx-cli` in three workflows; `install-tools.sh` and `doctor.sh`.
- **Tests and features:** e2e world, steps and features (`example_api_resource.feature`, `graphql.feature`, `http_errors.feature`, `api_docs.feature`, the database scenario of `health_check.feature`); `tests/support` fixture and repository.
- **Contracts and model:** both snapshots; `workspace.dsl` (database container, example group).
- **Profile:** `.claude/rust-sdlc.yml`: `persistence.engine: null`, `migrations_dir: null`, `offline_metadata: false`; six `make.*` keys set to `null` (`setup_test_db`, `create_migration`, `migrate`, `migrate_rollback`, `sqlx_prepare`, `validate_sqlx_metadata`); the required check "Persistence query metadata" dropped.
- **Documentation:** about 20 documents and 6 skills.
- **Order:** removal stories A (code) and B (stack and gates) are green in `make ci` only together [V by reading, not run]. They are one story with two commits.
- **After the removal:** liveness `/ping` is unchanged. Readiness `/api/health` reports the cache, SQS and the event stream.

## 5. Protected files

The rule (`AGENTS.md:16`): never change an item of `protected-config.toml` to obtain a pass. A maintainer approves the change with the label `approved-config-change`, and the session is restarted with `ALLOW_PROTECTED_CONFIG_EDIT=1`. The hook refuses the edit otherwise (`scripts/claude/guard-protected-files.sh`) [V]. The exact list of changes is the "Protected Changes" section of `architecture.md`. In summary:

| Group | Files | Approval |
| --- | --- | --- |
| Preparatory pull request (story 3.1, maintainer) | `architecture.toml`: infrastructure `external` += `krafka`, `tokio`; application `external` += `serde_json`; application `dev_external` += `tokio-tungstenite`; `protected-config.toml`: drop `.sqlx` and `migrations` from `required_directories` | `approved-config-change`, its own pull request merged into `main` before story 3.2 |
| Removal A (story 5.1, operator session) | `scripts/check-docs.sh`, `tests/CLI/bats/check_docs.bats`, the hash in `make_general.bats:81-94`, `make_quality_gates.bats:168-172` | `approved-config-change` and `approved-breaking-change` |
| Removal B (story 5.1, operator session) | `protected-config.toml:32-33`; delete `scripts/setup-test-db.sh` and `scripts/check-sqlx-metadata.sh`; `make/gates.mk:14`; `scripts/with-load-test-stack.sh`; delete `make_database.bats`; `make_tests.bats`, `make_general.bats`, `make_execution_mode.bats`, `make_lifecycle.bats`, `make_load_tests.bats`, `make_security.bats`; `make/tool-versions.mk:16`; `.cargo/config.toml:5-6`; `architecture.toml:57`; `.claude/settings.json` | `approved-config-change` |

Avoided protected changes, with the design choice that avoids them:

| Protected change | Avoided by |
| --- | --- |
| a Kafka host port in the Bats lifecycle tests | the host listener lives in a compose file that `make start` does not read |
| the `DEPS` text | a new `ensure-kafka` target in `make/service.mk` |
| `test-support` allow-list | the Kafka helpers live in crates whose allow-list is `*` |
| a Kafka variable in `make_security.bats` | every Kafka variable has a default |
| `deny.toml` | no change expected [U] |

## 6. Evidence and what does not exist

Section 3 of the design input names the evidence:

- **Election measurements** (`evidence/election-bench/VERIFIED.md`): audited. They measured an election mechanism that this plan does not use. They are the reason the election was dropped, nothing more.
- **Lock measurements** (`evidence/lock-bench/RESULTS.md`): not audited. They measured locks (Redis and others) that this plan does not use. They are the reason the locks were dropped and must not be quoted as figures.
- **Store bench** (`evidence/store-bench/src/backends/krafka.rs`): a harness whose krafka reader works against Kafka 4.1.1 (no group, assign, seek to ListOffsets, long poll, read-committed). Smoke runs only; its matrix was never run.
- **Fan-out harness** (`evidence/hub-spike/`): for 10 000 connections. Built and reviewed, never run. No fan-out number exists.
- **Kafka probes** (`replan/probes/`): the functional facts of section 3 (behaviour, errors, recovery delays seen in single runs). They are not throughput or latency measurements of this design.

What does not exist:

- No event throughput, delivery latency, fan-out latency or capacity figure for this design.
- The capacity figure of the first plan (10 000 subscribers on one node, p99 below 100 ms at 100 events/s) is a design target (D18, NFR-1). This change does not measure it.
- The K6 smoke budgets of FR-37 are budgets, not measurements.

## 7. Risks

| Risk | Severity | Mitigation in the plan |
| --- | --- | --- |
| `krafka` is young (one author, pre-1.0, breaking minor releases) | high | exact pin `=0.32.0`, a domain port, read-only use, `rdkafka` named as the alternative (D9) |
| `cargo deny` never run on `krafka` and its tree (`ring` or `aws-lc-rs`, licences, duplicates) | medium | story 3.5 runs `make.check_security`; a refusal is escalated, `deny.toml` is not edited |
| The fake broker differs from a real broker (seek above the end, log start, no TLS) | medium | fault cases only; behaviour cases run against the real broker; never run so far |
| `poll` hides outages; backoff delays recovery by up to 30 s | high, handled | watermark reads on a separate handle every 1 s, re-seek rule (FR-21) |
| Silent jump to the end with the default offset reset | high, handled | `AutoOffsetReset::None`, cursor comparison in the service |
| `wildcard_enum_match_arm` on `#[non_exhaustive]` client enums | low | adapter maps errors in one place; checked when the adapter is written |
| The allow-list approval blocks most of the feature (the envelope parser needs `serde_json` in the application layer) | medium | stories that do not need it come first (epics waves 1 to 5); the entries go into the preparatory pull request of story 3.1, requested in wave 1 |
| The removal is approval-gated as a whole and needs two merges and two labels | medium | removal is the last epic, done in an approved operator session; the preparatory pull request of story 3.1 is separate |
| A required status check "Persistence query metadata" may wait for ever | medium | hand-off to the maintainer in story 5.1 |
| Template sync restores removed files | medium | `.templatesyncignore` entries in the same story |
| Coverage of shared and service code that only the example routes exercised | medium | rewritten tests on `/api/graphql`, `/api/health` and the event routes [U: not run] |
| No measurement of capacity or latency | accepted | stated as a design target; a follow-up issue |
| A topic recreated with the same name invalidates cursors silently | low | documented in `docs/streaming-protocol.md` (FR-15) |
| A partition count that grows while a node runs is not noticed | low | follow-up (FR-17) |

## 8. Open questions resolved

> Assumption: the facts tagged [V] in the three fact files are taken as given and not re-verified here, because the budget of this run was nearly spent; the [U] items are carried as risks or as checks inside the stories (story 3.5 runs `cargo deny` through `make.check_security`; the operator confirms the image digest of story 1.3 against the registry).

> Assumption: the election and lock measurements are cited only as the reason those mechanisms were dropped; the unaudited lock results are never quoted as figures.

> Assumption: the Kafka probes are functional evidence (behaviour, error kinds, recovery after outages), and the single-run delays they recorded are not presented as performance of this design.

> Assumption: the open points of the fact files (failover timing of a multi-broker cluster, `connect` against an address that never answers, a leader change) stay open; the design bounds every broker call by `KAFKA_REQUEST_TIMEOUT` so that none of them can hang a node.
