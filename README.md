[![SWUbanner](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/banner2-direct.svg)](https://supportukrainenow.org/)

# Rust Service Template

[![Quality](https://github.com/VilnaCRM-Org/rust-service-template/actions/workflows/quality.yml/badge.svg)](https://github.com/VilnaCRM-Org/rust-service-template/actions/workflows/quality.yml)
[![Tests](https://github.com/VilnaCRM-Org/rust-service-template/actions/workflows/tests.yml/badge.svg)](https://github.com/VilnaCRM-Org/rust-service-template/actions/workflows/tests.yml)
[![Contracts](https://github.com/VilnaCRM-Org/rust-service-template/actions/workflows/contracts.yml/badge.svg)](https://github.com/VilnaCRM-Org/rust-service-template/actions/workflows/contracts.yml)
[![Security](https://github.com/VilnaCRM-Org/rust-service-template/actions/workflows/security.yml/badge.svg)](https://github.com/VilnaCRM-Org/rust-service-template/actions/workflows/security.yml)
[![codecov](https://codecov.io/gh/VilnaCRM-Org/rust-service-template/graph/badge.svg)](https://codecov.io/gh/VilnaCRM-Org/rust-service-template)
[![Latest release](https://img.shields.io/github/v/release/VilnaCRM-Org/rust-service-template)](https://github.com/VilnaCRM-Org/rust-service-template/releases/latest)
[![License](https://img.shields.io/github/license/VilnaCRM-Org/rust-service-template)](LICENSE)

`rust-service-template` is the VilnaCRM baseline for backend services written in Rust. It is the Rust counterpart of `php-service-template`: a Cargo workspace with one crate per bounded-context layer, a REST and GraphQL API on axum, PostgreSQL through sqlx, contract snapshots, architecture guards, quality gates behind one command surface, and guidance for AI coding assistants. A downstream service inherits the platform concerns instead of rebuilding them.

## Baseline

- Rust 1.99.1, pinned in `rust-toolchain.toml`, in a workspace with domain, application and infrastructure crates per bounded context
- REST and GraphQL endpoints with generated OpenAPI and GraphQL contract snapshots under `.github/`
- PostgreSQL with migrations and compile-time checked queries
- An architecture gate (`make architecture`) that keeps domain crates free of frameworks
- Unit, integration, end-to-end, mutation and load tests
- A Make command surface that runs natively or in containers
- Workflows for quality, tests, contracts, change control, security, releases and template updates

## Supported host platforms

| Platform | Status |
| --- | --- |
| Linux x86_64 | Verified in CI |
| Linux aarch64 | Supported through `RUN_IN_CONTAINER=1`; not verified |
| macOS | Supported through `RUN_IN_CONTAINER=1`; not verified |
| Windows | Supported through WSL 2; not verified |

## Quick start

Check the host, then set up the repository:

```bash
make doctor
make bootstrap
```

When the stack is running, these addresses answer:

| URL | What it serves |
| --- | --- |
| <http://localhost:8081/api/docs> | OpenAPI documentation |
| <http://localhost:8081/api/graphql> | GraphQL endpoint |
| <http://localhost:8081/api/health> | Readiness report |
| <http://localhost:8081/ping> | Liveness probe |
| <http://localhost:8080> | Architecture model viewer |

[`docs/getting-started.md`](docs/getting-started.md) covers native mode, container mode, the development container, stopping and cleaning.

## AI workflow

The repository carries guidance for AI coding assistants in `AGENTS.md`, `CLAUDE.md` and `.claude/skills/`. The prompt pattern that works best is explicit and repository-aware:

> Read `AGENTS.md`, pick the matching skill, implement the change, and verify it with `make ci`.

The single list of verification targets is in the "Verification" section of [`AGENTS.md`](AGENTS.md). Other files link to it and do not repeat it.

## Everyday commands

`make help` prints every target. [`docs/make-targets.md`](docs/make-targets.md) describes each one, its class, what it needs and where its test is.

## CI model

Pull requests run the Quality, Tests, Contracts, Change control, Security, Repository and Load tests workflows, including the mutation tests of the changed code and the smoke load profile. A weekly schedule runs the full mutation suite, a clean bootstrap, the security gates and the dev-container build. The average, stress and spike load profiles run on demand (`make load-tests`). A release is cut from `main` when conventional commits call for one. Every workflow calls the same Make targets that run locally, so a green local run predicts a green pull request. [`docs/repository-settings.md`](docs/repository-settings.md) lists the required status checks and repository settings.

## Creating a service and receiving updates

Create a repository from this template and follow [`docs/new-service.md`](docs/new-service.md): set the package identity in the workspace manifest, adjust `bmalph/config.json` and the badges above, and choose one template-sync flavour. The two guides in the `.github` directory, `TEMPLATE_SYNC_APP.md` and `TEMPLATE_SYNC_PAT.md`, explain how updates of the template reach a service; each names the other by its bare file name, so deleting one flavour leaves no broken citation.

## Cloud load testing

The k6 scenarios live in `tests/Load/`. `make load-tests` runs them locally, and `make aws-load-tests` runs them on a temporary AWS instance that `make aws-load-tests-cleanup` removes. Profiles, budgets, the isolated environment and costs are in [`docs/load-testing.md`](docs/load-testing.md).

## Documentation

- [`docs/README.md`](docs/README.md) — index of all documents
- [`docs/getting-started.md`](docs/getting-started.md) — prerequisites and first request
- [`docs/architecture.md`](docs/architecture.md) — layers, crates, request flow and error model
- [`docs/adr/README.md`](docs/adr/README.md) — decision records
- [`docs/new-service.md`](docs/new-service.md) — creating a service from the template
- [`docs/environment.md`](docs/environment.md) — environment variables
- [`docs/make-targets.md`](docs/make-targets.md) — Make targets
- [`docs/operations.md`](docs/operations.md) — migrations, probes, logs and shutdown
- [`docs/troubleshooting.md`](docs/troubleshooting.md) — common failures
- [`docs/extension-seams.md`](docs/extension-seams.md) — where a service extends the template
- [`docs/load-testing.md`](docs/load-testing.md) — load tests
- [`docs/repository-settings.md`](docs/repository-settings.md) — repository settings
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — how to contribute
- [`SECURITY.md`](SECURITY.md) — how to report a vulnerability

## License

This project is released under [CC0-1.0](LICENSE).
