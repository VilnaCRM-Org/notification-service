# Getting started

This guide takes a fresh clone to a running service and a first request. Every command is a Make target; `make help` lists them all and [`make-targets.md`](make-targets.md) explains each one.

## Prerequisites

Required on the host in every mode:

- GNU Make 4.0 or newer (on macOS: `brew install make` and run `gmake`, or put its `gnubin` directory first on `PATH`), Bash 4.4 or newer, git 2.30 or newer and curl 7.76 or newer.
- Docker Engine 25.0 or newer, the Compose plugin 2.24 or newer and the Buildx plugin 0.12 or newer. The engine must be reachable (`docker info` succeeds).

Required for native mode only (the default):

- rustup 1.26 or newer.
- The toolchain pinned in `rust-toolchain.toml`: Rust 1.99.1, profile `minimal`, with the components `rustfmt`, `clippy` and `llvm-tools-preview`. Install it in the checkout before the first `make bootstrap`:

  ```bash
  rustup toolchain install 1.99.1
  rustup component add llvm-tools-preview --toolchain 1.99.1
  ```

  The first command is the one `make doctor` prints. It installs rustup's default profile, which includes `rustfmt` and `clippy` but not `llvm-tools-preview`; the coverage targets need that component, so the second command adds it. While the toolchain is missing, `make bootstrap` stops at its first line with `[MISSING] toolchain 1.99.1` when rustup's automatic installation is turned off (`rustup set auto-install disable`, or `RUSTUP_AUTO_INSTALL=0`) or cannot download the toolchain, for example without network access. With automatic installation on, which is rustup's default setting, the first rustup call of the doctor installs the toolchain with the profile and components of `rust-toolchain.toml` instead.

Every other tool is installed by `make bootstrap` at its pinned version.

Optional: Node and npm, only for the `bmalph-*` targets.

Supported host platforms: Linux x86_64 is verified in CI. Linux aarch64 and macOS work through container mode (`RUN_IN_CONTAINER=1`, below), and Windows works through WSL 2; none of the three is verified in CI.

## Check the host

```bash
make doctor
```

The first line names the platform. Every later line is `[OK]`, `[MISSING]`, `[OUTDATED]`, `[OPTIONAL]` or `[SKIP]`. A `[MISSING]` or `[OUTDATED]` line says which targets need the tool and how to install it, and the command exits with status 1. [`troubleshooting.md`](troubleshooting.md) covers the common findings.

## Bootstrap

```bash
make bootstrap
```

This checks the prerequisites, installs the pinned tools and the Bats libraries, activates the git hooks, builds the images, starts the environment and waits until it is healthy, and prints the entry-point URLs. No environment file has to be created: `.env` and `.env.test` are committed and hold local-only placeholders (see [`environment.md`](environment.md)).

The first start is slow because the development image compiles the workspace; `make start` waits at most 900 seconds.

## First request

When `make bootstrap` has finished, the service listens on port 8081:

```bash
curl http://localhost:8081/ping
curl -i http://localhost:8081/api/health
```

`/ping` answers `pong` (liveness). `/api/health` answers 204 when the database, the cache and the message broker respond, and a 500 problem document that lists every failing dependency otherwise.

| URL | Content |
| --- | --- |
| `http://localhost:8081/api/docs` | REST API documentation |
| `http://localhost:8081/api/graphql` | GraphQL endpoint and, in `APP_ENV=dev`, the explorer |
| `http://localhost:8081/api/health` | health check |
| `http://localhost:8081/ping` | liveness |
| `http://localhost:8080` | architecture model (Structurizr viewer) |

The file [`requests.http`](requests.http) holds one ready-made request for every endpoint, for editors that read the `.http` format.

## Native mode

By default every target runs its tools directly on the host. The backing services (PostgreSQL and LocalStack) run in Docker; targets that need them start them through `ensure-deps` and `ensure-database` and never stop a service that already runs. To work without the containerised service:

```bash
make start-deps
make migrate
cargo run -p service -- serve
```

The binary loads `.env` itself, so no wrapper is needed. `make start-deps` starts only the services named in `DEPS`.

## Container mode

With `RUN_IN_CONTAINER=1` (or `true`), every target of class TOOL runs inside the development container, so the host needs no Rust toolchain:

```bash
export RUN_IN_CONTAINER=1
make bootstrap
make format-check
```

In this mode `make doctor` skips the toolchain checks and `make bootstrap` installs only Bats and the git hooks. The environment must be up (`make start`) before a TOOL target runs. The switch is ignored, and targets run natively, when `CI` is non-empty or when `IN_DEV_CONTAINER=1`. Reports still land in the checkout under `target/`.

## The development container

Two things carry the name.

- The `app` container of the compose environment. It runs the service with a file watcher, as your user, with the repository bind-mounted at `/app`. `make sh` opens a shell in it, `make logs` follows all logs and `make new-logs` follows only new lines.
- The editor development container defined in `.devcontainer/devcontainer.json`. It builds the same image and adds Docker inside, so it behaves like a host with the pinned toolchain: `make bootstrap`, `make start` and `make ci` run natively in it. Open the folder in a development-container-aware editor and the post-create command activates the hooks and runs the doctor.

## Structurizr and plain Compose

`make start` sets `COMPOSE_PROFILES=docs`, which starts the Structurizr viewer on port 8080. Running `docker compose up` without Make does not start the viewer, because the profile is set by the Makefile and not by a file. Use `make start`, or set `COMPOSE_PROFILES=docs` yourself.

## Stopping and cleaning

| Command | Effect |
| --- | --- |
| `make stop` | stops the development environment and keeps its containers |
| `make down` | removes the containers and the network; the volumes (database, caches) are kept |
| `make purge` | removes generated reports and load-test results; `make clean` is an alias |

`make down` is the safe way to free the ports. To remove the data volumes as well, use Docker Compose directly, for example `docker compose down --volumes`; the target never does it, so a restart keeps your database.

## Next steps

- Run every quality gate with `make ci`. The ordered list of gates is in the verification section of [`AGENTS.md`](../AGENTS.md#verification).
- Create your own service from the template: [`new-service.md`](new-service.md).
- Look up a variable or a target: [`environment.md`](environment.md), [`make-targets.md`](make-targets.md).
