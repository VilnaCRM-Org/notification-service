# Environment variables

This is the reference of every variable the template reads: the service settings, the compose and Make variables, and the two test seeds. Each row names the default, the consumer and the name the PHP service template used (empty when there was none). The document-drift check (`make docs-check`, rule 5) keeps this file equal to the sources: the help of `service serve`, `service migrate` and `service healthcheck`, the variables of the four compose files, the names in `.env` and `.env.test`, the variables the `Makefile` and its fragments declare with `?=`, and the keys of `[env]` in `.cargo/config.toml`.

A service that adds variables for its own adapters adds their rows here. The sync never overwrites this file.

## Service settings

"Required" means the command fails with exit status 2 when the variable is missing. Durations use the `humantime` syntax (`500ms`, `2s`, `1m`).

| Name | Type | Default | Required | Consumer | PHP name |
| --- | --- | --- | --- | --- | --- |
| `APP_ENV` | `dev`, `test` or `prod` | `prod` | no | `service` (the GraphQL explorer is on in `dev` only); compose | `APP_ENV` |
| `APP_HOST` | IP address | `0.0.0.0` | no | `service serve`; `service healthcheck` (probes this address, or the loopback address when this one is `0.0.0.0` or `::`) | none (Caddy) |
| `APP_PORT` | integer 0 to 65535 (0 = any free port, for tests) | `8081` | no | `service serve`, `service healthcheck`; compose | listener `:8081` in the Caddyfile |
| `LOG_LEVEL` | `tracing` filter directives | `info` | no | `service serve` (the other commands write plain text) | none |
| `DATABASE_URL` | `postgres://` or `postgresql://` URL | none | yes for `serve`, `migrate`, `migrate revert`, `check-config` | `service`; `sqlx-cli`; `#[sqlx::test]`; the end-to-end world | `DB_URL` |
| `DATABASE_MAX_CONNECTIONS` | integer 1 to 100 | `10` | no | `service` | none |
| `DATABASE_ACQUIRE_TIMEOUT` | duration | `2s` | no | `service` | none |
| `DATABASE_STATEMENT_TIMEOUT` | duration, `1ms` to `24d` (`0`, a value below `1ms` and a value above `24d` are rejected) | `5s` | no | `service`, including `service migrate` (see [Migrations and rollback](operations.md#migrations-and-rollback)) | none |
| `DATABASE_WAIT_TIMEOUT` | duration | `30s` | no | `service migrate` (one attempt per second until it elapses) | entrypoint loop, 60 tries |
| `AWS_REGION` | string | none | yes for `serve`, `check-config` | `service` | `AWS_SQS_REGION` |
| `AWS_ENDPOINT_URL_SQS` | `http://` or `https://` URL | unset (regional AWS endpoint) | no | `service` | `AWS_SQS_ENDPOINT_BASE` + `LOCALSTACK_PORT` |
| `AWS_ACCESS_KEY_ID` | string | unset (the provider's default credential chain: role credentials) | no; both keys set or both unset | validated as a pair by `service serve` and `check-config`; the AWS SDK credential chain reads the value | `AWS_SQS_KEY` |
| `AWS_SECRET_ACCESS_KEY` | string | unset (the provider's default credential chain: role credentials) | no; both keys set or both unset | validated as a pair by `service serve` and `check-config`; the AWS SDK credential chain reads the value | `AWS_SQS_SECRET` |
| `SQS_CONNECT_TIMEOUT` | duration | `500ms` | no | `service` | none |
| `SQS_OPERATION_TIMEOUT` | duration | `2s` | no | `service` (attempt and operation timeout) | none |
| `SQS_MAX_ATTEMPTS` | integer 1 to 5 | `1` | no | `service` | none |
| `HEALTH_PROBE_TIMEOUT` | duration | `3s` | no | `service` | none |
| `HTTP_REQUEST_TIMEOUT` | duration | `10s` | no | `service` | none |
| `HTTP_MAX_BODY_BYTES` | integer 1 024 to 10 485 760 | `65536` | no | `service` | none |
| `SHUTDOWN_GRACE_PERIOD` | duration | `10s` | no | `service` | none |

Cross-field rules checked at start-up: `HEALTH_PROBE_TIMEOUT >= DATABASE_ACQUIRE_TIMEOUT`; `HEALTH_PROBE_TIMEOUT >= SQS_OPERATION_TIMEOUT`; `HTTP_REQUEST_TIMEOUT > HEALTH_PROBE_TIMEOUT`; the two AWS key variables are both set or both unset (the error names the missing one). A configuration problem never prints the value of a variable.

## Compose and local stack

| Name | Type | Default | Required | Consumer | PHP name |
| --- | --- | --- | --- | --- | --- |
| `DB_VERSION` | string | `16` | no | compose | `DB_VERSION` |
| `DB_HOST` | host name | `localhost` | no | `.env` interpolation; compose sets `database` inside containers | host part of `DB_URL` |
| `DB_PORT` | integer | `5432` | no | compose (the host port of `database`); `.env` interpolation; compose sets `5432`, the port inside the compose network, in the `app` and `migrate` containers, so a changed host port does not change their URL | `DB_PORT` |
| `DB_NAME` | string | `app` | no | compose; `.env` interpolation | `DB_NAME` |
| `DB_USER` | string | `app` | no | compose; `.env` interpolation | `DB_USER` |
| `DB_PASSWORD` | string | `local-dev-only` | no | compose; `.env` interpolation | `DB_PASSWORD` |
| `LOCALSTACK_HOST` | host name | `localhost` | no | `.env` interpolation; compose sets `localstack` inside containers | `AWS_SQS_ENDPOINT_BASE` |
| `LOCALSTACK_PORT` | integer | `4566` | no | compose (the host port of `localstack`); `.env` interpolation; compose sets `4566`, the port inside the compose network, in the `app` container | `LOCALSTACK_PORT` |
| `STRUCTURIZR_PORT` | integer | `8080` | no | compose | `STRUCTURIZR_PORT` |
| `GITHUB_TOKEN` | token | unset | no | compose: the source of the build secret `github_token` of the development image; raises the rate limit of the tool downloads | none |

## Test seeds

The two seeds are test settings, not service settings. Cargo passes them to every test process from `[env]` of `.cargo/config.toml`, and a value set by the caller wins. `make` forwards both into the development container. The other two rows are build settings from the same `[env]` table.

| Name | Type | Default | Required | Consumer | PHP name |
| --- | --- | --- | --- | --- | --- |
| `TEST_SEED` | unsigned 64-bit integer | `20261110`, the seed recorded in `.cargo/config.toml` | no | `test_support::seed::TestRng` | none (the PHP data was unseeded) |
| `PROPTEST_RNG_SEED` | unsigned 64-bit integer | `20261110`, recorded in `.cargo/config.toml` | no | property tests (`proptest` reads it) | none |
| `SQLX_OFFLINE` | `true` | `true` from `.cargo/config.toml` | no | sqlx macros at build time | none |
| `SWAGGER_UI_DOWNLOAD_URL` | `vendored-only` | `vendored-only` from `.cargo/config.toml` | no | the build script of `utoipa-swagger-ui`: with the `vendored` feature it uses the embedded assets and ignores the value; without the feature the build stops instead of downloading Swagger UI (FR8) | none |

## Make variables

Every variable a caller may set, except `PROJECT_NAME` (see below the table), is declared with `?=` in the `Makefile` or in one of its two fragments, so the caller's value always wins. Pass them on the command line (`make start BUILD_IMAGE=0`) or through the environment. `TEST_SEED` and `PROPTEST_RNG_SEED` are not Make variables; see the seeds above.

| Name | Default | Consumer | PHP name |
| --- | --- | --- | --- |
| `COMPOSE_PROJECT_NAME` | `$(PROJECT_NAME)`; a value set by the caller is kept | exported to Compose | none |
| `RUN_IN_CONTAINER` | empty | `1` or `true`: targets of class TOOL run in the `app` container | none |
| `CI` | from the environment | non-empty: native mode is forced, `COMPOSE_PROFILES` defaults to empty and `CARGO_BUILD_WARNINGS=deny` is exported | `CI` |
| `IN_DEV_CONTAINER` | from the environment (`1` in the `app` container of the development environment) | native mode is forced; `ensure-deps` and `ensure-database` start nothing | none |
| `CARGO_BUILD_WARNINGS` | `deny` when `CI` is set and inside `make ci`; otherwise from the environment | Cargo's `build.warnings`; unset again for cargo-mutants | none |
| `DOCKER` | `docker` | exported; container engine command, also used by the scripts | `DOCKER` |
| `DOCKER_COMPOSE` | `docker compose` | exported; Compose command, also used by the scripts | `DOCKER_COMPOSE` |
| `HOST_UID` | `$(shell id -u)` | exported; user of the development container and of `docker run` tools | none |
| `HOST_GID` | `$(shell id -g)` | exported; group of the development container and of `docker run` tools | none |
| `COMPOSE_PROFILES` | `docs` (empty when `CI` is set) | exported; `docs` starts the Structurizr viewer | none |
| `APP_IMAGE` | `$(PROJECT_NAME)-app:prod` | exported; image name used by the compose files | none |
| `DEV_IMAGE` | `$(PROJECT_NAME)-app:dev` | exported; image name used by the compose files | none |
| `BUILD_IMAGE` | `1` | exported; `0`: `start`, `build-prod-image` and the isolated-environment wrapper use images that are already built and pass `--no-build` | none |
| `DEPS` | `database localstack` | compose services that `start-deps` starts | none |
| `base_ref` | `origin/main` | base of `validate-configuration`, `openapi-diff`, `graphql-diff` and `mutation-tests-diff` | `base_ref` |
| `CONFIG_CHANGE_APPROVED` | empty | `true` waives the approval of a protected-configuration change (set by CI from the label) | none |
| `BREAKING_CHANGE_APPROVED` | empty | `true` waives the approval of a breaking API change (set by CI from the label) | none |
| `LOAD_TEST_PROJECT` | `$(PROJECT_NAME)-load` | exported; compose project of the isolated production-image environment | none |
| `LOAD_TEST_APP_PORT` | `18081` | exported; host port of the service in that environment | none |
| `SCHEMATHESIS_URL` | `http://localhost:$(LOAD_TEST_APP_PORT)` | target of Schemathesis; when the caller sets it nothing is started | `SCHEMATHESIS_URL` |
| `scenario` | empty | parameter of `execute-load-tests-script` | `scenario` |
| `runSmoke` | `true` | parameter of `execute-load-tests-script` | `runSmoke` |
| `runAverage` | `true` | parameter of `execute-load-tests-script` | `runAverage` |
| `runStress` | `true` | parameter of `execute-load-tests-script` | `runStress` |
| `runSpike` | `true` | parameter of `execute-load-tests-script` | `runSpike` |
| `LOCAL_MODE_ENV` | `false` | `true`: the AWS load scripts talk to LocalStack | none |
| `region` | empty (the script default applies) | `aws-load-tests`, passed as `-r` | none |
| `ami` | empty (the script default applies) | `aws-load-tests`, passed as `-a` | none |
| `instance_type` | empty (the script default applies) | `aws-load-tests`, passed as `-t` | none |
| `instance_tag` | empty (the script default applies) | `aws-load-tests`, passed as `-i` | none |
| `role` | empty (the script default applies) | `aws-load-tests`, passed as `-o` | none |
| `branch` | empty (the script default applies) | `aws-load-tests`, passed as `-b` | none |
| `repo_url` | empty (the `origin` remote is used) | `aws-load-tests`, passed as `-u` | none |
| `security_group` | empty (the script default applies) | `aws-load-tests`, passed as `-s` | none |
| `load_target` | empty (the script default applies) | `aws-load-tests`, passed as `-l` | none |
| `name` | empty | parameter of `create-migration` (snake_case description) | none |
| `group` | empty | parameter of `new-context` | none |
| `context` | empty | parameter of `new-context` | none |
| `MUTANTS_SHARD` | empty | `k/n`: adds `--shard k/n` to `mutation-tests` and `mutation-tests-diff` | none |
| `MUTANTS_IN_PLACE` | empty | set by CI only: exactly `1` adds `--in-place` to `mutation-tests` and `mutation-tests-diff`, so cargo-mutants builds in the checkout and reuses the restored `target/` (NFR40); any other value adds nothing. Leave it unset locally: in place, cargo-mutants rewrites source files of the working tree while it runs | none |
| `ARCH_ARGS` | empty | test hook of the negative Bats tests (`architecture`) | none |
| `DENY_ARGS` | empty | test hook of the negative Bats tests (`check-security`) | none |
| `BATS_PATH` | `tests/CLI/bats` | test hook: the suite `bats` runs | none |
| `BATS_ARGS` | empty | extra arguments of `bats`, for example a tag filter | none |
| `DOCKERFILES` | `Dockerfile` | test hook: the files `lint-docker` checks | none |
| `IMAGE_TO_SCAN` | empty | test hook: the image `image-scan` scans instead of the production image | none |
| `OPENAPI_DIFF_IMAGE` | the pinned oasdiff image | overrides the image of `openapi-diff`; the PHP variable name is kept | `OPENAPI_DIFF_IMAGE` |
| `CI_STAGES` | the seven stages of `make ci` | stages run by `ci`; lets Bats test the aggregate without running it | none |
| `BMALPH_PLATFORM` | `claude-code` | parameter of the bmalph targets (`claude-code` or `codex`) | none |
| `BMALPH_DRY_RUN` | `false` | `true`: the bmalph targets preview without changing files | none |

Four names that the `Makefile` computes are not rows of the table, because the drift check keeps the table equal to the `?=` declarations. `PROJECT_NAME` is the Compose project name and the prefix of the image names: the caller's `COMPOSE_PROJECT_NAME` when set, else the checkout directory name, lower-cased with characters outside `a-z0-9_-` replaced by `-`; a value from the command line or the environment wins. `EXEC` is the prefix of every class TOOL command: empty, or `docker compose exec -T --user <uid>:<gid> -e … app`; it is computed and not meant to be set. `EXEC_ENV` lists the variables that `EXEC` forwards into the container (`TEST_SEED PROPTEST_RNG_SEED CONFIG_CHANGE_APPROVED CARGO_BUILD_WARNINGS DATABASE_STATEMENT_TIMEOUT`; the last one lets `DATABASE_STATEMENT_TIMEOUT=30m make migrate` reach the container). `TEST_ENV` is the prefix of commands that need the test configuration (`env -u DATABASE_URL scripts/with-env.sh .env .env.test --`): the caller's `DATABASE_URL` is removed first, so the test targets always use the database of `.env.test` (see "Committed environment files").

## PHP-to-Rust name mapping

| PHP name | Rust name | Note |
| --- | --- | --- |
| `DB_URL` | `DATABASE_URL` | the URL is built from the `DB_*` parts in `.env` |
| `AWS_SQS_REGION` | `AWS_REGION` | the standard AWS name |
| `AWS_SQS_KEY`, `AWS_SQS_SECRET` | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | the standard AWS names; both or neither |
| `AWS_SQS_ENDPOINT_BASE` and `LOCALSTACK_PORT` | `AWS_ENDPOINT_URL_SQS` | one URL; in `.env` it is built from `LOCALSTACK_HOST` and `LOCALSTACK_PORT` |
| `APP_ENV`, `DB_VERSION`, `DB_PORT`, `DB_NAME`, `DB_USER`, `DB_PASSWORD`, `LOCALSTACK_PORT`, `STRUCTURIZR_PORT` | same names | kept |

PHP variables that are not carried over: `APP_SECRET`, `AWS_SQS_VERSION`, `EMAIL_QUEUE_NAME`, `MESSENGER_TRANSPORT_DSN`, `CADDY_MERCURE_JWT_SECRET`, `SERVER_NAME`, `APP_RUNTIME_HTTP_ENV`, `XDEBUG_MODE`, `KERNEL_CLASS`, `PANTHER_*`, `SYMFONY_DEPRECATIONS_HELPER`, `LOAD_TEST_CONFIG`, `HTTP_PORT`, `HTTPS_PORT` and `HTTP3_PORT`. The framework, the mail queue, Mercure, the Caddy server and the PHP debugger have no counterpart in the Rust service.

## Committed environment files

`.env` and `.env.test` are tracked and are not git-ignored. They hold placeholders for the local compose stack only: nothing in them is a secret, and a value must never be reused outside a developer machine.

- `.env` holds the development defaults, for the compose stack and for `cargo run -p service -- serve`.
- `.env.test` holds only the values that differ from `.env` (`APP_ENV=test`, `LOG_LEVEL=warn` and a `DATABASE_URL` that ends in `_test`), so each value is defined in one place. It is always read after `.env`.
- `${NAME}` references are resolved by the service (`dotenvy`), by Docker Compose and by `scripts/with-env.sh`, and in all three a value already present in the process environment wins. That is how the same file works on a host (`localhost`) and inside a container, where compose sets `DB_HOST=database` and `LOCALSTACK_HOST=localstack` and the in-network ports `DB_PORT=5432` and `LOCALSTACK_PORT=4566`, so a host port changed in `.env` changes only what compose publishes.
- The test targets run with the variables of `.env` and then of `.env.test` exported (`env -u DATABASE_URL scripts/with-env.sh .env .env.test --`); a variable the caller has already set overrides both, with one exception: `DATABASE_URL` is removed from the caller's environment first, so the test targets always use the `_test` database of `.env.test` and never the database that a `DATABASE_URL` exported in the shell names (for `sqlx-cli`, for example). To move the test database, set `DB_PORT`, `DB_HOST` or `DB_NAME`: the URL of `.env.test` is built from them. `scripts/setup-test-db.sh`, which drops and recreates its database, additionally refuses with status 2 any database whose name does not end in `_test`. The `Makefile` includes neither file.
- `scripts/with-env.sh` reads a value like the service and Compose do, for the forms the three read the same way: `NAME=value`, a value in double quotes (`\"`, `\\` and `\n` are unescaped) or in single quotes (literal), and an inline comment after whitespace (`NAME=value # comment`). It refuses with status 2 the forms they read differently: an unquoted value that holds whitespace, a quote or a backslash, a `$` that does not start `${NAME}`, an unknown escape, a quote that does not close on its line, and a comment right after the `=`. Write such a value in quotes.
- Sourcing a file in a shell (`set -a; . ./.env`) overrides the caller's values and must not be used.
- The production image contains no env file. Supply every required variable at run time: `DATABASE_URL` and `AWS_REGION` have no default.
