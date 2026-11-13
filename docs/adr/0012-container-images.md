# 0012. Container images

Planning decisions: DD-19 (isolated production-image environment for conformance and load tests), DD-20 (container images).

## Context

Conformance and load tests must run against the production image, not a debug build, without touching development data, and the images must build quickly and reproducibly. Both base images must be literal, pinned references so that dependency tooling can update them and the invariants gate can read them.

## Decision

One `Dockerfile` with six stages: `base`, `chef`, `planner`, `builder`, `dev` and `prod`. The builder is `rust:1.99.1-slim-trixie` and the runtime is `gcr.io/distroless/cc-debian13:nonroot`, both written as literal `FROM` references with tag and digest, and no build argument replaces an image. The dependency layer uses cargo-chef. The production image runs as user `65532:65532` with entry point `/app/service` and a healthcheck in exec form that makes the binary probe itself, because the image has no shell. The development image carries the toolchain and the tools, runs as the invoking user and keeps cargo state outside `/app`.

Schemathesis and the load tests run against the production image in a compose project of its own, built from `docker-compose.yml` plus `docker-compose.load_test.override.yml`: its own network and volumes, no published database or LocalStack port, and the service published on `LOAD_TEST_APP_PORT`. A script builds the stack, waits for health, prepares the database, runs the command and always removes the project with its volumes. When the caller sets `SCHEMATHESIS_URL`, no environment is started and that URL is tested.

## Consequences

The invariants gate fails when the builder tag, `rust-toolchain.toml` and `rust-version` differ. A build behind a TLS-intercepting proxy replaces the builder for one build through a named build context, without changing a tracked file. A full check runs next to a started development environment because the isolated project uses its own port. A finding in the image scan without a fix is accepted only as a dated entry in `.trivyignore`.

Alternatives rejected: port 8081 for both environments, a third compose project for Schemathesis, a third-party cargo-chef base image, a musl variant and cache mounts only.
