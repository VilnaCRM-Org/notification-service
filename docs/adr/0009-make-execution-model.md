# 0009. Make execution model

Planning decisions: DD-01 (Make execution model), DD-02 (content and order of the full check).

## Context

The PHP template runs most targets inside a container and decided that with a variable set only in CI, so a local run and a CI run could differ. Developers on a machine without a Rust toolchain still need a supported path, and the full check must be exactly as strict locally as in CI.

## Decision

Every target runs its tools natively in the current environment by default. The opt-in variable `RUN_IN_CONTAINER` runs tool targets in the development container instead. The opt-in is ignored when `CI` is non-empty or when the shell is already inside the development container. Targets that drive Docker themselves always run where Make runs. Test targets wrap their command in a small script that exports the variables of the committed environment files in order, so the same line yields a database host that fits the place it runs in. The Makefile includes no environment file.

`make ci` runs the environment check and then seven ordered stages: cheap repository checks, static analysis, tests with coverage, contract checks, changed-code mutation, delivery checks that need images, and the Bats suite. It stops at the first failure and prints `CI checks successfully passed!`. It sets build warnings to deny for its own run, so a local run is as strict as hosted CI. Every gate is check-style and changes no tracked file; fix-style targets are separate and are never called by the full check, a hook or a workflow. Backing services are started through prerequisites with `--no-recreate`, so a running service is never altered.

## Consequences

Container mode needs no toolchain on the host, which is the supported path on Linux aarch64 and macOS. `make ci` needs Docker, network access and a fetched `origin/main`. The suites run once under instrumentation and the Bats stage holds the negative test of every gate rather than a second passing run. Parallel stages were rejected because their output interleaves and they share build locks. The list of gates is shown once, in the verification section of `AGENTS.md`, and checked against the Makefile.

Alternatives rejected: container by default, re-invoking Make inside the container, a second variable for CI, and leaving slow gates out of the local run.
