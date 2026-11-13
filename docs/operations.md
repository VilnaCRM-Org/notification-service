# Operations

Runbooks for operating the service and for maintaining the repository that hosts it. Every command named here is a Make target (see `docs/make-targets.md`) or a command of the `service` binary. Variable names and defaults are listed in `docs/environment.md`.

## Contents

- [Migrations and rollback](#migrations-and-rollback)
- [Health-probe runbook](#health-probe-runbook)
- [Logs and request ids](#logs-and-request-ids)
- [Shutdown sequence](#shutdown-sequence)
- [Toolchain update procedure](#toolchain-update-procedure)
- [Tool-version review](#tool-version-review)
- [First measurements](#first-measurements)
- [Build caches in CI](#build-caches-in-ci)
- [Accepted findings: `.trivyignore` and `.gitleaksignore`](#accepted-findings-trivyignore-and-gitleaksignore)

## Migrations and rollback

Migrations are plain SQL files in `migrations/` at the workspace root, written as reversible pairs: `<UTC timestamp>_<snake_case_name>.up.sql` and `.down.sql`. They are embedded in the `service` binary, so the production image needs neither `sqlx-cli` nor the directory. The service never migrates on start: `serve` starts without touching the schema, and migrating is a separate step.

| Task | Command | Result |
| --- | --- | --- |
| Apply every pending migration to the development database | `make migrate` | `applied <n> migration(s)` on stdout; nothing changes when none is pending |
| Revert the most recently applied migration | `make migrate-rollback` | `reverted <version>` on stdout, or `nothing to revert` |
| Create a new reversible pair | `make create-migration name=<snake_case_description>` | an empty `.up.sql` and `.down.sql` |
| Refresh the committed query metadata after a query changed | `make sqlx-prepare` | updated files in `.sqlx/`, committed with the migration |

Both migration commands wait for the database first: one attempt per second until `DATABASE_WAIT_TIMEOUT` elapses (default 30 seconds). Each failed attempt prints `waiting for database (attempt <n>): <error>` on stderr. When the database stays unreachable the command exits with status 1 and prints the connection error. An invalid configuration exits with status 2.

Migrations run under `DATABASE_STATEMENT_TIMEOUT` like every statement of the service (default 5 seconds, accepted range 1 millisecond to 24 days; PostgreSQL treats 0 as "no limit", so 0 is rejected). A statement that runs longer is cancelled with `canceling statement due to statement timeout`, the command exits with status 1 and the compose service `app`, which waits for `migrate`, does not start. Before a long migration such as an index build or a backfill, raise the variable for the migrate step only, for example `DATABASE_STATEMENT_TIMEOUT=30m make migrate`, or set it in the environment of the `migrate` container.

In a deployed environment the production image runs the same step with its first argument: `migrate` applies and `migrate revert` reverts. The compose files run it as the one-shot service `migrate`, which the service `app` waits for.

Rules:

1. An applied migration is never edited. A migration that has reached `main` may already have run in other environments; correct it with a new migration.
2. Every migration belongs to one bounded context and touches only that context's tables.
3. Write the `.down.sql` file so that it undoes the `.up.sql` file, and run `make migrate-rollback` followed by `make migrate` once before you commit the pair.
4. Commit `.sqlx/` together with the migration when a query changed, or the offline build fails.

Rollback of a deployment:

1. Stop or scale down the new version when it cannot run against the older schema.
2. Run `migrate revert` once per migration of the failed release, newest first. Each call reverts exactly one migration.
3. Deploy the previous image.
4. Check the readiness endpoint `GET /api/health` (see the next section).

A `.down.sql` file that drops a column or table destroys data. Take a backup before a rollback in production and decide whether to restore it.

## Health-probe runbook

`GET /api/health` answers `204` with an empty body when every probe succeeds. Otherwise it answers `500` with a problem document of type `/problems/dependency-unavailable`. Its `failures` member holds one entry per failing dependency, and `detail` joins the messages with a semicolon and a space. The messages are fixed and identical in every environment. The underlying cause is never returned: it is a log line `health probe failed` at `WARN` with the fields `dependency` and `cause` (the whole error chain, outermost first, joined by a colon and a space, for example `probe failed: dispatch failure: <reason>`), and it carries the request id of the probe request.

`GET /ping` is the liveness check. It touches no dependency, so a dependency outage never restarts a healthy process. Use `/ping` for container and process liveness and `/api/health` for readiness behind a load balancer.

All probes run concurrently and each has a deadline of `HEALTH_PROBE_TIMEOUT` (default 3 seconds). A probe that hangs counts as failed, and its `cause` is `timed out after 3s`. The adapters have shorter limits of their own (database pool acquisition 2 seconds, SQS operation 2 seconds), so a refused connection reports its real cause and the outer deadline only catches a dependency that hangs.

To see the cause, filter the log for the probe line:

```sh
docker compose logs app | jq -c 'select(.message == "health probe failed")'
```

### `Database is not available`

Wire identifier: `dependency` is `database`. The probe runs `SELECT 1` on the connection pool.

Likely causes:

- the database is stopped, restarting or unreachable from the service (network, firewall, wrong host or port);
- `DATABASE_URL` is wrong, or the credentials were rotated;
- the pool is exhausted: every connection is busy for longer than `DATABASE_ACQUIRE_TIMEOUT` (default 2 seconds), which is a capacity problem rather than an outage;
- the database accepts connections but the statement exceeds `DATABASE_STATEMENT_TIMEOUT`.

Checks:

1. Read the `cause` of the `health probe failed` line with `dependency` equal to `database`.
2. From the same network as the service, connect with `psql` using the host, port and user of `DATABASE_URL`. Never paste the URL into a ticket: it contains the password.
3. Compare `DATABASE_MAX_CONNECTIONS` with the database's connection limit and with the number of running instances.
4. Look for long-running or locked statements in `pg_stat_activity`.
5. After the database is back, the pool reconnects by itself (the pool is lazy); no restart is needed.

### `Cache is not working`

Wire identifier: `dependency` is `cache`. The probe stores the key `health_check` with the value `ok` and a 60-second lifetime through the `Cache` port, reads it back and compares.

Likely causes:

- the shipped adapter is the in-process `MokaCache`, which cannot be unreachable, so a failure means the write or the comparison failed: look at `cause` for a cache error or a value mismatch;
- after a service replaced the adapter with an external one (see `docs/extension-seams.md`): the cache server is down, refuses the credentials or is slower than the probe deadline.

Checks:

1. Read the `cause` of the `health probe failed` line with `dependency` equal to `cache`.
2. For an external cache, connect to it from the service's network with its own client and run a write and a read.
3. Check that the adapter's connection and operation timeouts are below `HEALTH_PROBE_TIMEOUT`; configuration validation enforces this for the shipped settings, and a new adapter must keep the rule.

No response of the service depends on cache content, so this failure reports a degraded dependency but loses no data.

### `Message broker is not available`

Wire identifier: `dependency` is `message_broker`. The probe calls `ListQueues` with a result limit of one on the SQS endpoint. The call reads account state and creates nothing; it needs only the permission `sqs:ListQueues`.

Likely causes:

- `AWS_ENDPOINT_URL_SQS` points to an endpoint that is down (in development: the `localstack` container is still starting or stopped) or is blank where the regional endpoint is meant;
- `AWS_REGION` is wrong;
- the credentials are missing, expired or lack `sqs:ListQueues` (in AWS the default credential chain supplies role credentials; locally `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` come from the environment, both set or both unset);
- the network blocks the endpoint, or the call exceeds `SQS_OPERATION_TIMEOUT` (default 2 seconds) or `SQS_CONNECT_TIMEOUT` (default 500 milliseconds).

Checks:

1. Read the `cause` of the `health probe failed` line with `dependency` equal to `message_broker`; it holds the SDK error chain.
2. In development run `docker compose ps` and check that `localstack` is healthy; LocalStack needs 3 to 6 seconds after a cold start.
3. From the service's network run the AWS CLI against the same endpoint, for example `aws sqs list-queues --max-items 1 --endpoint-url "$AWS_ENDPOINT_URL_SQS"`.
4. In AWS check the role policy for `sqs:ListQueues`.

### Several failures at once

When more than one probe fails, `detail` lists all messages in registration order (database, cache, message broker) and `failures` holds one entry for each. Work through each section above independently. A body that contains the message of a healthy dependency is a defect; report it.

### Starting without dependencies

`serve` does not wait for dependencies. With the database and the SQS endpoint down, `/ping` answers at once and `/api/health` answers with the problem document within about 2 seconds, naming both. This is the intended behaviour, not a failure of the service.

## Logs and request ids

`serve` writes one JSON object per line on stdout and nothing else on stdout or stderr. The other commands write plain text. Keys:

| Key | Present | Content |
| --- | --- | --- |
| `timestamp` | always | RFC 3339, UTC, microseconds |
| `level` | always | `ERROR`, `WARN`, `INFO`, `DEBUG` or `TRACE` |
| `message` | always | fixed text from the list below |
| `target` | always | Rust module path, or `audit` for audit entries |
| event fields | per message | flattened into the top level |
| `span` | inside a request | the innermost open span: `{"name":"http.request","http.request.method":…,"http.route":…,"url.path":…,"request_id":…}`, or a span opened inside the request (an SQS call opens its own) |
| `spans` | inside a request | every open span, outermost first; the first element is always the `http.request` span with the `request_id` |

Fixed messages:

| Message | Level | Fields | Meaning |
| --- | --- | --- | --- |
| `invalid configuration` | ERROR | `variable`, `reason` | one entry per problem; `serve` then exits with status 2 |
| `listening` | INFO | `addr`, `env`, `version` | start-up marker |
| `request completed` | INFO | `http.response.status_code`, `duration_ms` | the access log, exactly one per request, `/ping`, 404 and 405 included |
| `request failed` | ERROR | `error` | the error chain behind a 5xx problem |
| `panic` | ERROR | `panic` | message and location; the request gets a 500 problem and the process keeps running |
| `health probe failed` | WARN | `dependency`, `cause` | see the health-probe runbook |
| `event publication failed` | ERROR | `event_name`, `event_id`, `error` | one per failing subscriber, `error` is the whole error chain joined by a colon and a space; the request still succeeds |
| `domain event` | INFO, target `audit` | `event_name`, `event_id`, `aggregate_id`, `occurred_at` | the audit subscriber |
| `shutdown signal received` | INFO | `signal` | start of the shutdown sequence |
| `grace period elapsed, forcing shutdown` | WARN | `grace_period` | `serve` then exits with status 1 |
| `stopped` | INFO | none | `serve` then exits with status 0 |
| `serve failed` | ERROR | `error` | bind or runtime failure; `serve` then exits with status 1 |

Lines written outside a request (start-up, shutdown) carry no request span; the AWS SDK writes a few start-up entries at `debug` level inside spans of its own.

The level filter is `LOG_LEVEL`. It takes `tracing_subscriber::EnvFilter` directives: a level such as `debug`, or per-target levels such as `info,sqlx=warn,audit=info`. An invalid directive is a configuration error. Request and response bodies, request headers other than the request id, and `DATABASE_URL` are never logged.

### Request ids

The header is `x-request-id`. An inbound value is accepted when it matches `^[A-Za-z0-9._-]{1,128}$`; any other value is dropped and a UUID is generated. The id is echoed on every response, including 404, 405, panic and timeout responses, and it is the field `request_id` of the first element of `spans` of every log line of the request, also of the lines written inside a span the request opened (at `debug` level the SQS client writes such lines). To follow one request:

```sh
docker compose logs --no-log-prefix app | jq -c 'select(any(.spans[]?; .request_id == "<id>"))'
```

A client that reports a problem should quote the `x-request-id` of the response; the bug-report template asks for it.

## Shutdown sequence

1. At t = 0 the process receives SIGTERM or SIGINT and logs `shutdown signal received`. The listener closes: new connections are refused within about 10 milliseconds. In-flight requests continue.
2. When the last in-flight request has completed, the process logs `stopped` and exits with status 0. An idle service exits within 2 seconds.
3. At t = `SHUTDOWN_GRACE_PERIOD` (default 10 seconds) with a connection still open, the process logs `grace period elapsed, forcing shutdown` at `WARN`, drops the remaining connections and exits with status 1 within 2 seconds.

An exit status 1 after a deployment therefore means that at least one request was cut off by the end of the grace period. Check the access log for requests that did not log `request completed`, and consider raising `SHUTDOWN_GRACE_PERIOD` or lowering `HTTP_REQUEST_TIMEOUT` (default 10 seconds). A bind failure or runtime failure also exits with status 1 and logs `serve failed`. Status 2 is always an invalid configuration.

Set the orchestrator's termination timeout to at least 15 seconds, so that SIGKILL does not arrive before the grace period ends. The container healthcheck runs `["/app/service", "healthcheck"]` (the image has no shell) every 10 seconds with a timeout of 3 seconds and 3 retries; it probes `/ping` on the address in `APP_HOST` (the loopback address when `APP_HOST` is `0.0.0.0` or `::`, as in the image), so a service bound to one specific address is probed on that address.

## Toolchain update procedure

The Rust toolchain version is held in three places. `make validate-invariants` fails when they differ, and all three are protected configuration:

| Place | What it holds |
| --- | --- |
| `rust-toolchain.toml` | the channel, for every local and CI build |
| `Dockerfile` | the tag (and digest) of the builder image, a literal `FROM` reference |
| `Cargo.toml` | `rust-version` in `[workspace.package]` |

Dependabot opens one pull request for the toolchain file and one for the images. Do not merge them separately: each leaves the three places unequal and fails the invariant gate.

1. Create one branch from `main`.
2. Take the toolchain change of the Dependabot toolchain pull request and the builder-image change (tag and digest) of the Docker pull request, and set `rust-version` to the same version. Close both Dependabot pull requests when yours is open.
3. Review `make/tool-versions.mk` for the new toolchain (see the next section), because Dependabot cannot update that file.
4. Run `make ci`. Fix new compiler or clippy findings; do not weaken a lint or add a suppression to pass.
5. Open one pull request, title `chore(#<issue>): update Rust to <version>`, and ask a maintainer to add the label `approved-config-change`. The change-control job fails without it because the files are protected.
6. Merge with squash. Releases are not triggered by a `chore` commit.

## Tool-version review

Every tool the Make targets and the scripts use has its version, or its image reference with tag and digest, in `make/tool-versions.mk`. Dependabot cannot update this file, so a maintainer reviews it with each toolchain update and at least once a quarter.

CI installs the cargo tools with `taiki-e/install-action`, pinned by commit, with `fallback: none`. A version may therefore be raised only to a version that the file `manifests/<tool>.json` of the pinned install action lists. The pinned commit lists every cargo tool at its pinned version, and `typos` up to 1.52.0.

1. Look up the newest release of the tool and read `manifests/<tool>.json` at the pinned action commit.
2. If the newer version is listed, change it in `make/tool-versions.mk` and in every `tool:` entry of the workflows. `make validate-invariants` fails when a `tool:` entry differs from the file.
3. If it is not listed, first bump the install action (a pull request that changes the pin and its tag comment), then raise the tool.
4. For an image, take the tag and the digest from the registry and keep both in the reference.
5. Run `make ci` and open a pull request with the label `approved-config-change`.

## First measurements

These tables record what the first runs cost, so that later changes can be judged against them. The local values were entered by Story 10.5; the hosted-runner values are those of the first complete pipeline run, on commit `db7fe08` of the bootstrap pull request on 2026-11-11, of the first run of the scheduled workflow on `main`, on commit `3e0bd3e` on 2026-11-11, of a dispatch of that workflow on the branch of the follow-up pull request #2, on commit `027e1f1` on 2026-11-11, of the first warm pull-request runs, on commits `e9eef29` and `95547ba9506b6e54d04747203b1a1a06ba693904` of pull request #2 on 2026-11-12, and of a second dispatch of the scheduled workflow on that branch, on commit `da5ea9c` on 2026-11-12 (run 43151995703). No documentation-only pull request has run yet: its duration (budget 5 minutes) is an estimate, below.

Conditions of the local runs, 2026-11-11: a virtual machine with 4 vCPUs, 15 GB of memory and Linux 6.18, a fresh clone in a temporary directory (project name `service-clone`), no build directory and no compiler cache. Cargo ran with `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_DEV_DEBUG=0` and everything else at its default. The hosted jobs use the same two settings: the pull-request jobs that run cargo on the runner receive both from the toolchain action (`actions-rust-lang/setup-rust-toolchain` sets them), and the scheduled clean-clone job `clean-bootstrap-ci`, which does not use that action, sets both in the `env` of its workflow. The pinned tools were already installed on the machine, so `make bootstrap` did not compile them. Both container images were built beforehand from the same commit and the runs used `BUILD_IMAGE=0`, so the image builds are not part of any duration below. The development image had been built earlier the same day, and its inputs (`make/tool-versions.mk`, `scripts/install-tools.sh`, `scripts/install-bats.sh` and the base image) had not changed since. The machine was shared with other work (load average 3.84 at the start of `make bootstrap`). The commit is `d152806`.

Local runs, on that machine:

| Command | Date | Duration | Budget of the PRD | Budget met | Notes |
| --- | --- | --- | --- | --- | --- |
| `make bootstrap` | 2026-11-11 | 116 s (1 min 56 s) | `GET /ping` within 15 minutes | yes, without tool and image builds (see the conditions) | clean clone, no build directory; `GET /ping` answered 200 right after the command returned; it compiled no tool and built no image (see the conditions) |
| `make ci` | 2026-11-11 | 1482 s (24 min 42 s) | within 45 minutes on 4 vCPUs | yes, without the production image build (see the conditions) | after `make bootstrap`, cold build directory; exit 0, 297 Bats tests, all `ok`; stages below |
| `make start` | 2026-11-11 | 12 s | within 60 s with images and cache present | yes | after the `make ci` run, images and build output present, the stack stopped before |
| `make ci base_ref=HEAD` (Story 10.4, for comparison) | 2026-11-11 | 1575 s (26 min 15 s) | within 45 minutes on 4 vCPUs | yes | warm compiler cache and build directory in the working tree, commit `ecbca71`, 295 Bats tests; this run is not comparable with the cold run above: it used other cargo settings and ran beside other work |
| `make mutation-tests` (Story 9.3, all 8 shards, one after the other) | 2026-11-11 | 339 s to 1146 s per shard, see below | none | not applicable | 665 mutants: 409 caught, 256 unviable, none missed or timed out |
| `npx --yes @devcontainers/cli@0.90.0 build`, the two features of `.devcontainer/devcontainer.json` only (partial) | 2026-11-11 | 79 s | none | not applicable | partial evidence: `build` was replaced by a development image built earlier on this machine (plus the network settings the machine needs), so only the two feature layers were built; common-utils 2.7.1 created the user `dev` (uid and gid 1000, groups `dev` and `docker`) and docker-in-docker 4.1.3 with `moby: false` installed Docker 29.9.3, Compose v2.40.3 and buildx v0.39.0 on Debian trixie; the first full build of the committed definition, by the job `devcontainer-build` of the scheduled run on commit `3e0bd3e` (below), failed in the common-utils feature, which this run did not show because the kernel setting `fs.protected_regular` is 0 on this machine; the build of the fixed definition passed on a hosted runner (run 43119953189, below) |

The budgets are the ones of the PRD. All three are met by the local runs; no budget, threshold or timeout was changed. The local runs are not a measurement of a cold hosted runner: they excluded the installation of the tools and both image builds (see the conditions). The hosted runs of commit `db7fe08` measured the two image builds, each in a job that starts without any cache (the image caches are saved on `main` only, and `main` had not run these workflows):

| Part excluded locally | Hosted job and step | Duration |
| --- | --- | --- |
| cold build of the development image | `Bats: surface`, build step 16:29:11Z to 16:32:05Z | 2 min 54 s |
| cold build of the production image | `Smoke load tests`, build step 16:28:40Z to 16:32:40Z | 4 min 0 s |

Estimate for a cold hosted runner, from these figures and the local runs (a sum of figures from two machines with 4 vCPUs each, not a measurement): `make bootstrap` about 116 s plus 2 min 54 s, that is about 4 min 50 s, plus the installation of the pinned tools, which no run has measured (budget 15 minutes); `make ci` about 24 min 42 s plus 4 min 0 s, that is about 28 min 42 s (budget 45 minutes). The first run of the scheduled job `clean-bootstrap-ci`, which runs both commands on a clean clone on a hosted runner, measured them and confirms the budgets of both (see the scheduled run below).

Stages of the `make ci` run, from the timestamps of the log (the stage that was running was sampled every 2 s, so each boundary is accurate to about 2 s):

| Stage | Duration |
| --- | --- |
| 1 `ci-preflight` | 50 s |
| 2 `ci-static-analysis` | 3 min 53 s |
| 3 `ci-tests` | 3 min 11 s |
| 4 `ci-contracts` | 28 s |
| 5 `ci-mutation` | 2 s (empty diff, no mutants) |
| 6 `ci-delivery` | 54 s |
| 7 `ci-cli` | 15 min 24 s |
| Total | 24 min 42 s (1482 s) |

The six Bats groups run one after the other inside stage 7, files in alphabetical order. The duration of a test is the time between its result line and the one before it, so the first test of a file also carries its `setup_file`. Each file belongs to the group of its `file_tags` line.

| Group | Tests | Duration |
| --- | --- | --- |
| `surface` | 78 | 6 min 9 s |
| `repository` | 144 | 3 min 0 s |
| `load` | 24 | 2 min 22 s |
| `tests` | 16 | 1 min 53 s |
| `quality` | 24 | 1 min 5 s |
| `contracts` | 11 | 57 s |
| Total | 297 | 15 min 26 s |

The sum is two seconds above the stage because the first Bats line is counted from the start of the command.

Mutation run of Story 9.3 on commit `3650cd4`, on the same machine with its own PostgreSQL and LocalStack, the shards run one after the other with the mutation slot of the machine: 665 mutants in 8 shards of 84 (the last one 77), all of them tested, none missed, none timed out. The duration of a shard from its start to its exit, which includes waiting for a free slot and the unmutated baseline build of the shard itself (339 s for the first shard, falling to 11 s for the last as the build directory warmed):

| Shard | Mutants | Caught | Unviable | Duration |
| --- | --- | --- | --- | --- |
| 0/8 | 84 | 47 | 37 | 1146 s |
| 1/8 | 84 | 38 | 46 | 819 s |
| 2/8 | 84 | 30 | 54 | 556 s |
| 3/8 | 84 | 58 | 26 | 358 s |
| 4/8 | 84 | 30 | 54 | 371 s |
| 5/8 | 84 | 70 | 14 | 433 s |
| 6/8 | 84 | 70 | 14 | 339 s |
| 7/8 | 77 | 66 | 11 | 375 s |

On the bootstrap pull request the hosted workflow counts 697 mutants for commit `d152806` (every file is new against `main`, which holds only the root commit) and divides them into shards of 150, which gives 5 shards. The 665 mutants above belong to the earlier commit `3650cd4`. The durations on 2026-11-11 for commit `d152806` are in the table of the pull-request jobs below. The planning job divides the mutants into shards of 150 (at least 1 and at most 8 shards); that divisor was not changed. The mean time per mutant was not computed: the local shards waited for slots and shared the machine, so a mean from them would not be a stable figure for the divisor.

Smoke load test: the `health` profile ran inside `make ci` (stage 6) with a p99 of 14.79 ms against the budget of 500 ms that `tests/Load/config.json.dist` held at the time of the run. The budget shipped since then is 50 ms, derived from the hosted-runner measurements; a p99 of 14.79 ms is within it (see `docs/load-testing.md`).

Pull-request jobs on a hosted runner, commit `db7fe08` of the bootstrap pull request, 2026-11-11, from the start and end times of the latest check run of each name. No Rust or image cache was restored, because the caches are saved on `main` only, so these are cold figures:

| Job | Date | Duration | Notes |
| --- | --- | --- | --- |
| `format-check` | 2026-11-11 | 17 s | check `Formatting check` |
| `static-analysis` | 2026-11-11 | 2 min 11 s | check `Static analysis` |
| `tests` | 2026-11-11 | 4 min 6 s | check `Tests and coverage` |
| `e2e-tests` | 2026-11-11 | 2 min 38 s | check `End-to-end tests` |
| `mutation-shard` (longest shard) | 2026-11-11 | 11 min 46 s | shard 1/5 of commit `db7fe08`; the other four took 6 min 0 s, 7 min 18 s, 8 min 37 s and 9 min 8 s. Earlier, on commit `d152806`, shard 1/5 took 15 min 32 s and the other four 5 min 28 s, 7 min 32 s, 7 min 49 s and 9 min 0 s |
| `bats-group` (longest group) | 2026-11-11 | 10 min 19 s | group `surface`, which includes the cold build of the development image (2 min 54 s); the other groups took 9 min 0 s (`contracts`), 8 min 38 s (`load`), 7 min 2 s (`tests`), 4 min 41 s (`quality`) and 3 min 56 s (`repository`) |
| `schemathesis` | 2026-11-11 | 4 min 34 s | check `Schemathesis` |
| `smoke-load-tests` | 2026-11-11 | 5 min 21 s | check `Smoke load tests`, which includes the cold build of the production image (4 min 0 s) |
| `image-scan` | 2026-11-11 | 5 min 30 s | check `Production-image scan` |
| `docs-check` | 2026-11-11 | 1 min 41 s | check `Documentation drift` |

The whole pull request on that commit: the first check started at 16:28:08Z, the last check other than the mutation check (`Makefile tests (Bats)`) ended at 16:39:01Z, 10 min 53 s later, and the mutation check (`Mutation tests`) ended at 16:43:53Z, 15 min 45 s after the first start. This is within the cold budget of 40 minutes, the mutation check apart, and within the 45 minutes per shard of the mutation check; the Bats requirement of 20 minutes, read per group because the six groups are separate jobs, is met by the longest group with 10 min 19 s.

The warm budget of 20 minutes was measured on pull request #2, the first pull request after the push of `3e0bd3e` to `main` had saved the caches. The figures come from the check suites and check runs of each commit (GitHub API, `commits/<sha>/check-suites` and `commits/<sha>/check-runs`). The push time is the creation time of the first check suite of the commit, and the end is the end of the last of the 36 required checks of `docs/repository-settings.md`. All 36 ran on both commits. `Protected configuration` failed on both, because the pull request changes protected files and carries no `approved-config-change` label yet; that is the gate working, and its time counts like that of any other check.

| Commit | Date | Push (first check suite) | Last required check | Duration | Budget met | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| `e9eef29` | 2026-11-12 | 00:29:14Z; the first check started at 00:29:21Z | `Makefile tests (Bats)`, 00:38:14Z | 9 min 0 s | yes (20 minutes) | warm: the toolchain steps restored the entries saved by the push of `3e0bd3e` with `full match: true` (for example `v0-rust-tests-...` in `Tests and coverage` and `v0-rust-mutation-...` in `Mutation shard 0/1`); 50 check runs: 47 jobs of the seven pull-request workflows, 2 code-scanning results and the skipped review bot; the mutation check `Mutation tests` was among them and ended at 00:33:37Z (4 min 23 s), so the figure is the same with and without it; the longest job was `Bats: surface` (8 min 39 s), the first recorded exception of NFR40 |
| `95547ba9506b6e54d04747203b1a1a06ba693904` | 2026-11-12 | 01:16:22Z; the first check started at 01:16:29Z | `Makefile tests (Bats)`, 01:25:43Z | 9 min 21 s | yes (20 minutes) | partly warm: this commit renamed the shared keys `mutation-in-place` and `bats-tests-in-place`, under which nothing was saved yet (`No cache found.`), so `Mutation shard 0/1` and `Bats: tests` ran cold; `Mutation tests` ended at 01:21:09Z (4 min 47 s); the longest job was again `Bats: surface` (9 min 1 s); 50 check runs, as above |

Estimate for a documentation-only pull request (budget 5 minutes), not a measurement. When the change detection gives `code=false`, only the seven `Changes (...)` jobs, `Secrets scan`, `Repository invariants`, `Documentation drift`, `Repository lint`, `Pull-request title` and `Protected configuration` run. The aggregators `Mutation tests` and `Makefile tests (Bats)` pass as soon as their skipped jobs are resolved, and every other job is skipped. These jobs do the same work for a documentation-only change as for a code change. On the two commits above the longest of them took 49 s and 44 s (`Repository lint`), and the last of them ended 57 s (`e9eef29`) and 52 s (`95547ba9506b6e54d04747203b1a1a06ba693904`) after the push. `Changes (Tests)` ended 12 s after the push on both, and each aggregator started 2 to 3 s after its last need and took 3 to 4 s. A documentation-only pull request should therefore complete in about 1 minute, if runners are as available as on these runs. The first documentation-only pull request after the merge replaces this estimate with its measurement.

Scheduled run on a hosted runner: run 43107219243 of the workflow `Scheduled verification`, started by hand (`workflow_dispatch`) on `main` after the merge of the bootstrap pull request, commit `3e0bd3e`, 2026-11-11, on GitHub-hosted runners with the label `ubuntu-24.04`. The job `Clean bootstrap and make ci` (`clean-bootstrap-ci`) restores no cache (its log does not show the number of CPUs of the runner); its `make bootstrap` installed the pinned tools, built the development image and started the stack. Durations from the start and end times of the steps and of the job:

| Step of `clean-bootstrap-ci` | Duration | Budget of the PRD | Budget met | Notes |
| --- | --- | --- | --- | --- |
| `Bootstrap` (`make bootstrap`) | 447 s (7 min 27 s) | `GET /ping` within 15 minutes | yes | the application container reported healthy at the end of the step; its health check sends `GET /ping` |
| `Run every gate` (`make ci`) | 1828 s (30 min 28 s) | within 45 minutes | yes | exit 0, 378 Bats tests, all `ok` |
| `Require a clean working tree` | 0 s (start and end in the same second) | none | not applicable | passed: the two commands left the working tree unchanged |
| the whole job | 2294 s (38 min 14 s) | none | not applicable | from the start to the end of the job |

This run confirms the budgets of NFR35 for `make bootstrap` (15 minutes) and `make ci` (45 minutes) on a hosted runner, from a clean clone with no restored cache and no manual step (NFR34). It did not measure the third budget, `make start` with images and build output present (60 s), which only the local run above measured (12 s). The job therefore has two more steps between `Bootstrap` and `Run every gate`: `Stop the stack` (`make stop`) and `Start the stack and time it`. The second times `make start`, writes `make start: <seconds> s (budget 60 s)` to the job summary and fails above 60 s; the failure summary names either step. Both run before `make ci`, so a dispatch on a branch measures them too, although its `make ci` stops at the configuration guard (see the dispatch below). The next dispatch on the branch measured it on a hosted runner: run 43151995703 of `Scheduled verification` (`workflow_dispatch` on commit `da5ea9c`, 2026-11-12, runner label `ubuntu-24.04`), job `Clean bootstrap and make ci` (job 130963102019): `Bootstrap` 452 s, `Stop the stack` 3 s, and `Start the stack and time it` printed `make start: 12 s (budget 60 s)`. The third budget of NFR35 is met on a hosted runner as well. The job then stopped at `validate-configuration`, as every dispatch on that branch does (see below).

The mutation shards of the same run ran `make mutation-tests` over the whole workspace in 5 shards; every shard and the job `Mutation tests` passed: 699 mutants, 459 caught, 240 unviable, none missed or timed out. The duration is that of the job:

| Shard | Mutants | Caught | Unviable | Duration |
| --- | --- | --- | --- | --- |
| 0/5 | 140 | 58 | 82 | 9 min 17 s |
| 1/5 | 140 | 92 | 48 | 15 min 36 s |
| 2/5 | 140 | 72 | 68 | 7 min 6 s |
| 3/5 | 140 | 116 | 24 | 8 min 46 s |
| 4/5 | 139 | 121 | 18 | 8 min 19 s |

The job `Development container build` (`devcontainer-build`) of the same run failed after 2 min 55 s. The `dev` stage of the `Dockerfile` built; the common-utils feature then stopped with `/home/dev/.bashrc: Permission denied`, because the `dev` stage gave the home directory the sticky bit and the runner's kernel refuses root an append to a file of another user in such a directory (`fs.protected_regular`). The `Dockerfile` now gives the home directory mode `0777` without the sticky bit, and the build of the fixed definition passed on a hosted runner (run 43119953189, below).

Dispatch on the branch of pull request #2: run 43119953189 of `Scheduled verification`, started by hand (`workflow_dispatch`) on commit `027e1f1`, 2026-11-11, runner label `ubuntu-24.04`. Durations from the start and end times of the jobs and steps:

| Job | Duration | Result | Notes |
| --- | --- | --- | --- |
| `Development container build` (job 130857597533) | 2 min 55 s (175 s), of which the build step 2 min 51 s | passed | FR95 and FR118 for the build: the devcontainers CLI 0.90.0 built the `dev` stage of the `Dockerfile` (the tool installation `[dev 5/6]` 109.5 s; `[dev 6/6]` now gives `/home/dev` mode `0777`) and both features, common-utils in 16.8 s (`useradd` warns that `/home/dev` exists; no error) and docker-in-docker in 13.6 s (`docker-in-docker-debian script has completed!`); last line `{"outcome":"success",...}` |
| `Clean bootstrap and make ci` | 8 min 23 s | failed, as expected on a branch | `Bootstrap` passed in 472 s (7 min 52 s), within the 15 minutes of NFR35; `make ci` stopped after 11 s at `validate-configuration`: the guard compares the branch with `main`, where 13 protected files differ, and a dispatch carries no pull-request label (`[FAIL] Configuration validation failed`, `Approve with the label approved-config-change`); this is the guard working, not a defect |
| `Mutation shard 0/5` to `Mutation shard 4/5`, `Mutation tests` | 7 min 26 s to 15 min 54 s per shard | passed | over the whole workspace |
| `Security gates` | 2 min 24 s | passed | |
| `Code scanning (rust)`, `Code scanning (actions)` | 9 min 30 s, 52 s | passed | |

Since that run the job `devcontainer-build` also starts the container and checks it from inside (FR118). Each check is a step of its own, with its own log under `target/`, and the failure summary names the failing step and shows the end of its log:

1. `npx --yes @devcontainers/cli@0.90.0 up --workspace-folder .` starts the container with the docker-in-docker entry point and runs its `postCreateCommand`, `make install-hooks && make doctor`; the doctor requires `docker info` to answer.
2. `npx --yes @devcontainers/cli@0.90.0 exec --workspace-folder . docker info` reaches the inner container engine.
3. `npx --yes @devcontainers/cli@0.90.0 exec --workspace-folder . make format-check` runs a toolchain target with the pinned toolchain.
4. `npx --yes @devcontainers/cli@0.90.0 exec --workspace-folder . make start-deps` starts PostgreSQL and LocalStack through compose on the inner engine.

The CLI version of every step equals `DEVCONTAINER_CLI_VERSION` of `make/tool-versions.mk` (G31 rule (b)). Run 43119953189 had only the build step. Run 43151995703 (dispatch on commit `da5ea9c`, 2026-11-12, runner label `ubuntu-24.04`), job `Development container build` (job 130963101979), passed every step:

| Step | Duration | Result | Last lines of its log |
| --- | --- | --- | --- |
| Build the development container | 164 s | passed | `{"outcome":"success","imageName":["vsc-rust-service-template-..."],...}` |
| Start the development container (`up`) | 7 s | passed | `Running the postCreateCommand from devcontainer.json...`; the doctor printed `[OK]` for every tool, among them `Docker Engine 29.9.3`, `Docker Compose plugin 2.40.3`, `cargo, toolchain 1.99.1` and `cocogitto 7.0.0`; `{"outcome":"success",...,"remoteUser":"dev","remoteWorkspaceFolder":"/app"}` |
| Reach the container engine inside the container (`docker info`) | under 1 s | passed | `Server Version: 29.9.3` |
| Run a toolchain target inside the container (`make format-check`) | 1 s | passed | `cargo fmt --all --check`, exit 0 |
| Start the backing services through the inner engine (`make start-deps`) | 29 s | passed | `Container app-database-1  Healthy`, `Container app-localstack-1  Healthy` |

This is the hosted proof of FR118: in the container the toolchain targets work and the container engine is reachable.

## Build caches in CI

Every job that compiles Rust code or builds an image restores a cache that a run on `main` saved; pull requests restore and never save (NFR40). NFR40 names four exceptions, numbered as in open item 5 of the delivery document of the planning bundle: (1) the `surface` Bats group, which compiles inside the development container; (2) the weekly `devcontainer-build` job, which builds without a layer cache on purpose; (3) the weekly job `Clean bootstrap and make ci` (`clean-bootstrap-ci`) of `scheduled.yml`, which restores no cache because FR95 and NFR34 require its `make bootstrap` and `make ci` to start from a clean checkout with no restored cache; (4) for the rerun clause, on a rerun of a pull-request commit, the third-party crates that a mutation shard still compiles because its packages need them and the saver on `main` did not build them (see [the package selection](#what-a-restored-cache-cannot-cover-the-package-selection)). Images use the BuildKit layer cache of GitHub (`type=gha`, scopes `prod-image` and `dev-image`, written with `mode=max` on `main` only).

### Rust caches

The toolchain action (`actions-rust-lang/setup-rust-toolchain` 2.0.1) runs `Swatinem/rust-cache` 2.9.3. It caches `target/` and the registry, git checkouts and installed binaries of the cargo home under the key `v0-rust-<cache-shared-key>-Linux-x64-<hash 1>-<hash 2>`. Hash 1 covers the compiler version and every variable whose name starts with `CARGO`, `CC`, `CFLAGS`, `CXX`, `CMAKE` or `RUST`; hash 2 covers the manifests, `Cargo.lock`, `rust-toolchain.toml` and `.cargo/config.toml`. The only restore key is the key without hash 2, matched as a prefix, so a changed lock file restores the previous entry of the same shared key. Before saving, rust-cache removes the build output of the workspace crates and keeps that of the third-party crates.

rust-cache saves nothing when the restore was an exact hit (`Cache up-to-date.`). An entry is therefore never refreshed while the lock file and the toolchain stay the same. When what a saver builds changes, rename its shared key, or the old entry stays in use. For this reason `mutation` became `mutation-in-place` and `bats-tests` became `bats-tests-in-place` when cargo-mutants started building in the checkout. The entries saved under the old names held no build of the dependencies that the in-place builds need.

| Shared key | Saver (on `main`) | Other jobs that restore it |
| --- | --- | --- |
| `check` | `static-analysis` | `complexity`, `code-scanning` (scheduled) |
| `service-debug` | `check-requirements` | `contract-snapshots`, `docs-check` |
| `crate-isolation` | `crate-isolation` | none |
| `query-metadata` | `query-metadata` | none |
| `xtask` | `architecture` | `validate-configuration`, `architecture-model` |
| `tests` | `tests` | none |
| `e2e` | `e2e-tests` | none |
| `mutation-in-place` | `mutation-shard` with index 1, in `tests.yml` and `scheduled.yml` | `mutation-plan`, the other shards |
| `bats-<group>`, for the group `tests` `bats-tests-in-place` | `bats-group`, one key per group | none |

The jobs `crate-isolation` (`Crate isolation`) and `query-metadata` (`Persistence query metadata`) of `quality.yml` restored `check` and `service-debug` until pull request #2 and saved nothing. Their commands select other packages, features and targets than the savers of those keys, so they recompiled hundreds of third-party compile units on every run, a rerun of an unchanged commit included (see [the package selection](#what-a-restored-cache-cannot-cover-the-package-selection)). Each now saves a key of its own on `main`, and no other job restores it. The gate step writes its output to `target/crate-isolation.log` or `target/query-metadata.log` (with `set -o pipefail`, so the step keeps the status of the gate), and the step after it, which runs even when the gate failed, prints `Third-party crates compiled: <count>`: the `Compiling` and `Checking` lines without a path, after the colour codes are removed. rust-cache removes the files at the top of `target/` before it saves, so a restored entry never brings an old log back.

Cost against the repository's cache quota of 10 GB: two more entries on `main`, one per key, so the workflows now use 14 Rust cache keys instead of 12. A new entry exists only after a push to `main` has saved it, so its size is to be measured after the merge of pull request #2. The neighbouring entries, as the toolchain step of the two jobs printed when it restored them: `check` `Cache Size: ~422 MB (442408998 B)` and `service-debug` `Cache Size: ~372 MB (390088549 B)` (jobs 131262929871 and 131262929917). If the two new entries are of the same order, they add about 0.8 GB; that is an estimate, not a measurement.

### Mutation testing builds in the checkout

By default cargo-mutants copies the source tree to a scratch directory in the system temporary directory, without `target/` (its option `copy_target` is off by default), and builds there. The restored cache then cannot help: the hosted rerun of an unchanged commit compiled 321 third-party crates in `Mutation shard 0/1`, and the entry it restored, saved by a run on `main` that had also built in a scratch copy, held no dependency build output.

The CI jobs therefore set `MUTANTS_IN_PLACE: '1'`: the `mutation-shard` jobs of `tests.yml` and `scheduled.yml`, and the `bats-group` job of `tests.yml`. There, the Bats file `make_tests.bats` passes the value on to its G15 test, because every Bats test unsets the Make variables of the caller. Exactly the value `1` adds `--in-place` to the command of `mutation-tests` and `mutation-tests-diff`, and any other value adds nothing. It is not an argument hook: no other flag can reach cargo-mutants this way, and `make_tests.bats` checks this. cargo-mutants then builds in the checkout and reuses the `target/` that the cache restored. The timeouts do not change, because cargo-mutants derives the test timeout from the baseline test time and sets no build timeout here.

Keep the switch unset locally. In place, cargo-mutants writes each mutant into the source file and writes the original text back after the mutant, also when the build tool fails. The tree is the same at the end, but it is rewritten while the run lasts, and a run killed in between leaves a mutant behind. A property test that a mutant makes fail can also leave a `proptest-regressions/` file next to its source. A scratch copy collects the same file, which every mutant of the run then reads, so the outcomes do not depend on the mode. In place, the file stays in the working tree; the hosted checkout is discarded after the job.

The same selection was run both ways on 2026-11-12 with cargo-mutants 27.1.0: 23 mutants in four files, the fixture function of the G15 test appended to `xtask/src/cli.rs`. Both runs gave 17 caught, 2 missed, 4 unviable and no timeout, with the same outcome for every mutant and status 2 for both. The G15 test of `make_tests.bats` compiled 40 third-party crates in a scratch copy (24.9 s) and none in place with a warm `target/` (16.8 s).

### What a restored cache cannot cover: the package selection

Which third-party crates cargo builds, and with which features, depends on the packages that a command selects. cargo-mutants builds its baseline with `--package` for every package that has a mutant in the shard, and then each mutant with `--package` for its own package. The saver on `main` builds only the selections of its own shard. On the current tree the full run has 701 mutants in 5 shards, and the saver's shard holds the mutants of `health-check-application`, `health-check-domain`, `health-check-infrastructure` and `service`. A pull-request shard whose mutants lie elsewhere still compiles the crates that differ.

The table below gives the third-party compile units of one pull-request shard, counted from the `Compiling` lines without a path. Each count starts from a target directory that holds exactly what the saver builds: its baseline and one build per package, 508 compile units from an empty directory. The commands are the cargo-mutants build commands replayed in a scratch copy of commit `027e1f1` on 2026-11-12.

| Packages with mutants in the shard | Baseline | Per-package builds | Total |
| --- | --- | --- | --- |
| `service` | 0 | 0 | 0 |
| `health-check-domain` | 0 | 0 | 0 |
| `shared-application` | 0 | 0 | 0 |
| `xtask` | 17 | 0 | 17 |
| `shared-domain` | 15 | 0 | 15 |
| `shared-infrastructure` | 15 | 0 | 15 |
| `some-module-application`, `some-module-domain`, `some-module-infrastructure` | 49 | 8, 7, 13 | 77 |
| `service`, `shared-domain` | 13 | 0, 9 | 22 |

Before the switch, every shard compiled every third-party crate of its selection on every run.

A warm-up on the saver does not close the gap. It was measured with every package with mutants built alone, plus the whole workspace: that costs 213 compile units on the saver and raises the build output from 2.80 GB to 4.64 GB. After it, 4 of 9 sampled baselines of two or three packages still compiled 2, 14, 31 and 38 crates, and a shard can select any of the 2047 combinations of the 11 packages with mutants. Unifying features across the workspace needs the unstable `-Zfeature-unification`; stable cargo 1.99.1 ignores `resolver.feature-unification`.

The saver's own selections also go stale. When the mutant count moves the slice boundaries, the saver's shard holds other packages, but its entry is replaced only when the lock file or the toolchain changes.

This gap is the fourth recorded exception of NFR40, for its rerun clause: 0 to 77 third-party compile units per pull-request shard, against 321 to 345 before the switch, and 0 for the selections of the saver itself. A cache saved per pull request would close it, but each entry of about 800 MB would take a share of the repository's cache quota of 10 GB for every pull request and commit, and the quota would evict the entries of `main` that every other job restores.

The same cause made two jobs of `quality.yml` recompile on every run while they restored the entry of another job:

- `Crate isolation` restored `check`. Its saver, `static-analysis`, runs two clippy passes (`cargo clippy --locked --workspace --lib --bins --all-features` and `cargo clippy --locked --workspace --all-targets --all-features`, each followed by its lint options) and `cargo doc --locked --workspace --no-deps --document-private-items`, each over the whole workspace at once, so in each of them cargo builds a third-party crate with the union of the features that all crates of the workspace ask for. `make check-crate-isolation` runs `cargo hack check --workspace --each-feature --locked`, which checks each crate alone, once per feature set of the crate; the crates of the template declare no features of their own, so the hosted log shows one `cargo check --locked` per crate (`running ... on <crate> (1/13)` to `(13/13)`). Cargo resolves features per selection, so a third-party crate is built with the features that this one crate selects, without the other crates and without the dev-dependencies, and each such feature set is a compile unit of its own that the `check` entry does not hold.
- `Persistence query metadata` restored `service-debug`. Its saver, `check-requirements`, builds only the `service` binary. `make validate-sqlx-metadata` runs `cargo sqlx prepare --workspace --check -- --all-targets --all-features --locked`, which also builds the tests and the other targets of every crate, and with them the dev-dependencies, `cucumber` among them, that the `service-debug` entry does not hold.

The third-party compile units of the gate step, counted in the hosted logs as the new count step counts them (the `Compiling` and `Checking` lines without a path, after the colour codes are removed):

| Job | Commit | Restored entry | Third-party compile units |
| --- | --- | --- | --- |
| `Crate isolation` (job 130883700953) | `e9eef29` | `v0-rust-check-...`, `full match: true` | 224 (53 `Compiling`, 171 `Checking`) |
| `Persistence query metadata` (job 130883700960) | `e9eef29` | `v0-rust-service-debug-...`, `full match: true` | 354 (55 `Compiling`, 299 `Checking`) |
| `Crate isolation` (job 131262929871) | `6a9dd24` | the same entry of `check`, `full match: false` | 224 (50 `Compiling`, 174 `Checking`) |
| `Persistence query metadata` (job 131262929917) | `6a9dd24` | the same entry of `service-debug`, `full match: false` | 354 (56 `Compiling`, 298 `Checking`) |

On `e9eef29` the restore was an exact hit, so these are the counts that a rerun of an unchanged commit gave. On `6a9dd24` only `.cargo/config.toml` had changed since the entry was saved (one `[env]` line; no manifest and not `Cargo.lock`), so hash 2 of the key was `24b880c7` against `f2938adc` of the entry and the restore matched by prefix only. The counts are the same: what the two jobs missed was their own selection, not a change of the key files. Unlike the mutation shards, each of the two jobs runs one fixed command on every run, so an entry that the job saves itself holds exactly what that command builds. Since pull request #2, each job saves a key of its own on `main` (see [the Rust caches](#rust-caches)).

### Checking the rerun clause on hosted runners

The proof comes only from hosted logs. In the logs, `<hash>` stands for the two hashes of the key.

On the first push to `main` after a key was renamed or added:

- `Crate isolation`: toolchain step `No cache found.`. The step after `make check-crate-isolation` prints `Third-party crates compiled: <count>`, with a high count on this cold run. The post step of the toolchain action prints `... Saving cache ...` for `v0-rust-crate-isolation-Linux-x64-<hash>` and the size of the entry.
- `Persistence query metadata`: the same lines for `v0-rust-query-metadata-Linux-x64-<hash>`, after `make validate-sqlx-metadata`.
- `Mutation shard 0/<n>` (index 1), toolchain step: `No cache found.`. The mutation step's command ends in `--shard 0/<n> --in-place`. The step after it prints `Third-party crates compiled by cargo-mutants in this shard: <count>`, with a high count on this cold run. The post step of the toolchain action prints `... Saving cache ...` for `v0-rust-mutation-in-place-Linux-x64-<hash>`. The entry is far larger than the 121,575,024 bytes of the old `mutation` entry. The artifact `mutants-1` has no `Copied source tree` line in `debug.log`, and the `Compiling` lines of `log/baseline.log` name paths in the checkout.
- `Bats: tests`: `No cache found.` for `v0-rust-bats-tests-in-place-Linux-x64-<hash>`, the TAP line `# G15: <count> third-party crates compiled by cargo-mutants (MUTANTS_IN_PLACE=1)`, and `... Saving cache ...` at the end.
- A saver job that fails or is cancelled still saves, unless its restore was an exact hit. The toolchain action passes its default `cache-on-failure: true` to rust-cache, so the post step of rust-cache also runs after a failure, a cancellation or the job timeout (`post-if: success() || env.CACHE_ON_FAILURE == 'true'`). A missed mutant leaves the entry complete, because cargo-mutants tests every mutant before it exits 2. A job that was cancelled or timed out, or whose baseline failed, saves only what it built until then, and an exact hit never refreshes that entry. So check that the first saver run after a rename got to the end. Its mutation step prints the cargo-mutants summary `<count> mutants tested in <time>: ...`, which comes only after the last mutant, and `Bats: tests` prints the TAP line of G15. `Crate isolation` and `Persistence query metadata` got to the end when their gate step passed. If any of these is missing, rename the key again.

On a rerun of an unchanged pull-request commit, started after that push:

- `Crate isolation`: `Restored from cache key "v0-rust-crate-isolation-Linux-x64-<hash>" full match: true.` and `Third-party crates compiled: 0`.
- `Persistence query metadata`: `Restored from cache key "v0-rust-query-metadata-Linux-x64-<hash>" full match: true.` and `Third-party crates compiled: 0`.
- `Mutation plan` and every `Mutation shard k/n`: `Restored from cache key "v0-rust-mutation-in-place-Linux-x64-<hash>" full match: true.`. Each shard prints `Third-party crates compiled by cargo-mutants in this shard: 0` when its packages are among the saver's selections. Otherwise it prints the count of the table above for its selection, the same on the first attempt and on the rerun, because pull-request runs save nothing.
- `Bats: tests`: `Restored from cache key "v0-rust-bats-tests-in-place-Linux-x64-<hash>" full match: true.` and `# G15: 0 third-party crates compiled by cargo-mutants (MUTANTS_IN_PLACE=1)`. The G15 selection is the same on every run.

Some of this can be proven on the pull request that introduces the switch, before it is merged:

- The mutation shards run in place: the command ends in `--in-place`, `debug.log` has no `Copied source tree` line, and the paths of `log/baseline.log` lie in the checkout.
- The outcome counts match the earlier run of the same diff.
- The G15 test passes in place: its TAP line shows `MUTANTS_IN_PLACE=1`, and the test checks that the planted file and `git status` are unchanged afterwards.

Recorded on pull request #2, head `95547ba9506b6e54d04747203b1a1a06ba693904` (run 43133361295 of `Tests`, 2026-11-12), before the merge:

- `Mutation shard 0/1` (job 130902830997): the mutation step's command ends in `--shard 0/1 --in-place`. It printed `Found 8 mutants to test` and `8 mutants tested in 3m: 8 caught`: the same 8 mutants of `crates/service/src/config.rs` with the same outcomes as the run of the earlier head `e9eef29` without the switch (job 130883880975, `8 mutants tested in 2m: 8 caught`; no Rust file changed between the two heads). In the artifact `mutants-1`, `debug.log` has no `Copied source tree` line, the `build_dir` of the worker is the checkout, and the `Compiling` lines of `log/baseline.log` name paths in the checkout. The count step printed `Third-party crates compiled by cargo-mutants in this shard: 321`. That is a cold count: the toolchain step found no entry under the new key (`No cache found.`).
- `Bats: tests` (job 130902681311): `No cache found.` for the new key, the TAP line `# G15: 20 third-party crates compiled by cargo-mutants (MUTANTS_IN_PLACE=1)` followed by `ok 13 G15: a function whose tests miss a mutant makes mutation-tests-diff fail with a MISSED line`, and 24 of 24 tests `ok`.

The count 0 on a rerun can be shown only after the merge. Before it, no saved entry has the new names, and pull requests do not save. Caches saved by a push to the pull request's branch are not visible to its `pull_request` runs. The rerun result is to be filled from run `<id>`: the rerun of one unchanged pull-request commit across all seven pull-request workflows, after the first push to `main` has saved the renamed keys, with the count line of every shard and the G15 TAP line of `Bats: tests`.

The keys `crate-isolation` and `query-metadata` (pull request #2) are checked the same way, after the merge of pull request #2. Before it, the runs of the pull request find nothing under the two new names (`No cache found.`) and compile the selection of both jobs from nothing, so their count lines show cold counts. Check to run:

1. The first push to `main` after the merge: in `Crate isolation` and `Persistence query metadata`, the gate step passed and the post step of the toolchain action printed `... Saving cache ...` for `v0-rust-crate-isolation-Linux-x64-<hash>` and `v0-rust-query-metadata-Linux-x64-<hash>`, each with its `Cache Size`. Result: to be measured after the merge of pull request #2.
2. The rerun of one unchanged pull-request commit after that push, the run of the previous paragraph: both jobs print `Restored from cache key ... full match: true.` and `Third-party crates compiled: 0`. Result: to be measured after the merge of pull request #2.
3. The quota: in the same rerun, every other job that restores a key still restores the entry that `main` saved. A `No cache found.` there means that the quota of 10 GB evicted an entry of `main`. Then the two keys cannot be kept within the quota, and the counts of the table in [the package selection](#what-a-restored-cache-cannot-cover-the-package-selection) are recorded as a fifth exception of NFR40 instead. Result: to be measured after the merge of pull request #2.

## Accepted findings: `.trivyignore` and `.gitleaksignore`

A finding of a scanner is fixed, not accepted, unless no fix exists. An accepted finding is an entry with a date, a reason and a review date, so that nothing stays accepted without a decision.

`.trivyignore` lists vulnerability identifiers for the image scan (`make image-scan`, severities HIGH and CRITICAL). One identifier per line, with the comment lines directly above it:

```text
# 2026-11-10: libexample 1.2.3 in the runtime layer; no fixed package exists yet.
# The vulnerable function is not reachable from the service. Review by 2027-02-10.
CVE-2026-00000
```

`.gitleaksignore` lists fingerprints of accepted secret-scan findings, one fingerprint per line, with a dated comment above it. Take the fingerprint from the finding that `make secrets-scan` prints (`<commit>:<file>:<rule>:<line>` for a history finding, `<file>:<rule>:<line>` for a working file):

```text
# 2026-11-10: documented example token in an example file, not a credential. Review by 2027-02-10.
config/example.txt:generic-api-key:42
```

Rules for both files:

1. Add an entry only in a pull request that states why the finding cannot be fixed. A real secret is never accepted: revoke and rotate it, then remove it from the history.
2. Every entry carries the date of acceptance, the reason and the review date. The review date is at most three months away.
3. When the review date has passed, either remove the entry (the finding is fixed) or renew it with a new date and reason.
4. Both files belong to the service after a repository is created from the template, and template sync ignores them.
