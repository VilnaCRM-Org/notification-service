# Troubleshooting

Each section names a symptom and what to do about it.

## `make doctor` findings

`make doctor` prints one line per check; the command fails (exit status 1) while a `[MISSING]` or `[OUTDATED]` line remains.

| Line | Meaning | What to do |
| --- | --- | --- |
| `[MISSING] <tool>: needed by <targets>; <how to install>` | the tool is absent | install it as the line says, then run `make doctor` again |
| `[MISSING] toolchain <version>: needed by every cargo target; run: rustup toolchain install <version>` | the toolchain pinned in `rust-toolchain.toml` is not installed, and rustup does not install it on its own (automatic installation is off) or could not install it (for example without network access) | run the command the line names, then `rustup component add llvm-tools-preview --toolchain <version>` for the coverage targets (see [Prerequisites](getting-started.md#prerequisites)) |
| `[OUTDATED] <tool>: found <v>, need <v>; needed by <targets>; <how to install>` | the tool is older than the minimum, or differs from the pinned version | update it; for a pinned cargo tool run `make install` |
| `[OUTDATED] toolchain: found <v>, need <version>; needed by every cargo target; run: rustup toolchain install <version>` | the `rustc` that runs in the repository root prints another version than the one pinned in `rust-toolchain.toml` | run the command the line names. If the line stays after the install, something shadows the pin: check `which rustc` (it must be rustup's proxy under `~/.cargo/bin`), `echo $RUSTUP_TOOLCHAIN` (a value wins over `rust-toolchain.toml`) and `rustup override list` (an override for the repository directory wins too), then unset `RUSTUP_TOOLCHAIN`, remove the override (`rustup override unset` in the repository root) or take the other `rustc` off `PATH` |
| `[OPTIONAL] <tool>` | only some targets need it (for example npm for the `bmalph-*` targets) | ignore it unless you use those targets |
| `[SKIP] …` | container mode skips the toolchain checks | none; this is expected with `RUN_IN_CONTAINER=1` |

`make bootstrap` runs the prerequisite part of the doctor first, so a missing prerequisite stops it before anything is installed or started.

## `[OUTDATED] make: found 3.81` on macOS

macOS ships GNU Make 3.81 as `make`. `brew install make` installs the new one as `gmake`, and `make` stays old unless Homebrew's `gnubin` directory comes first on `PATH`. Run the targets with `gmake` (the doctor then checks the make that runs it), or put `$(brew --prefix)/opt/make/libexec/gnubin` first on `PATH`. The git hooks use `gmake` when it exists, or the make that the variable `MAKE` names.

## Docker Engine older than 25.0 or Compose older than 2.24

The compose files use the healthcheck key `start_interval`, which needs Docker Engine 25.0 or newer, and the `!reset` and `!override` tags, which need Compose 2.24 or newer. Older versions fail with a message about an unknown key or tag, and the doctor reports `[OUTDATED] Docker Engine` or `[OUTDATED] Docker Compose plugin`. Upgrade Docker, then run `make doctor`.

If the doctor says the engine is missing although Docker is installed, `docker info` fails: start the daemon, check your permissions on the Docker socket, or, inside a development-container shell, give the shell access to an engine. A HOST target in this state exits with status 2 and `This target needs a reachable container engine (docker).`

## First start is slow

The first `make start` builds the development image and compiles the workspace inside it, which takes several minutes; the `app` container is not healthy until the compile in the migration step is done. `make start` waits at most 900 seconds and then exits non-zero. Watch the progress with `make logs`. Later starts reuse the image layers and the build volume and take seconds. `make build` rebuilds the images from scratch and is only needed when you want to discard the cache.

## Port clashes

The environment publishes four ports on `127.0.0.1`: the service on `APP_PORT` (8081), PostgreSQL on `DB_PORT` (5432), LocalStack on `LOCALSTACK_PORT` (4566) and the Structurizr viewer on `STRUCTURIZR_PORT` (8080). When one is taken, `make start` or `make start-deps` fails with Docker's "port is already allocated" message.

Pick another port for the run, for example:

```bash
DB_PORT=15432 make start-deps
```

`.env` builds `DATABASE_URL` from `DB_PORT`, so the service and the tests follow the new port. To run a second environment next to the first, also give it its own project name (`PROJECT_NAME=other`), so the containers and volumes do not collide. Each project name gets its own containers, network and volumes.

## Stale `.sqlx`

The directory `.sqlx/` holds the offline query metadata the compiler reads (`SQLX_OFFLINE=true`). When a query or a migration changes and the metadata does not, `make validate-sqlx-metadata` fails and a build can fail with a message that a query is not in the cache. Regenerate it from the freshly migrated test database and commit the result:

```bash
make sqlx-prepare
```

An applied migration is never edited; add a new one with `make create-migration name=<snake_case_description>`.

## Restoring after an interrupted Bats run

Negative tests plant a violation in a tracked file, run a gate and restore the file in `teardown`. Bats runs `teardown` when a test fails or is interrupted, but a killed process can leave a planted change behind. Every copy is recorded in `target/bats-restore/` with a manifest, and the next Bats run replays the manifest first. Run `make bats` again, or any single Bats file, and then check `git status`: the tracked files must be back in their committed state. Planting refuses to touch a file with uncommitted changes, so your own work is never overwritten; the test is skipped with a message instead.

If a file is still modified after a full run, restore it with `git restore <file>` after you have confirmed that the change is not yours.

## Rate-limited tool installation

`make install` runs `scripts/install-tools.sh`, which fetches release archives from GitHub. Anonymous requests are rate limited. Set a token in the environment and run it again:

```bash
export GITHUB_TOKEN=<a token with no scopes>
make install
```

A tool that is already installed at its pinned version is skipped, so a repeated run only fetches what is missing. When no prebuilt binary exists for the platform, the script says so and compiles the tool with `cargo install --locked`, which takes longer.

## Building the images behind a TLS-intercepting proxy

On a network whose proxy intercepts TLS, the build of the images fails with a certificate error because the pinned builder image does not trust the proxy's certificate authority. Do not change a tracked file. Build a base image that carries the certificate, then replace the pinned builder for one build with a BuildKit named context:

```bash
docker build \
  --build-context <builder reference of the Dockerfile>=docker-image://<derived image> \
  --target dev --tag <project name>-app:dev .
docker build \
  --build-context <builder reference of the Dockerfile>=docker-image://<derived image> \
  --target prod --tag <project name>-app:prod .
```

`<builder reference of the Dockerfile>` is the exact image reference of the builder `FROM` line, and `<derived image>` is your image built `FROM` that builder with the certificate added. Once both images exist, run the targets with `BUILD_IMAGE=0`, which makes them use the images that are already built:

```bash
BUILD_IMAGE=0 make start
```

Targets whose tool containers download at run time (the linters that install packages, the image scan) need the proxy variables and the certificate inside the container; set `DOCKER` to a wrapper that adds them to `docker run`.
