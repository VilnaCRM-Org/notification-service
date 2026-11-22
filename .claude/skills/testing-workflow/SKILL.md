---
name: testing-workflow
description: Use when writing, running or debugging tests at any level.
---

# Testing Workflow

## Use This Skill When

- A unit, integration, end-to-end or command-line test is written, run or debugged.
- A coverage or mutation result needs interpretation.

## Commands

- `make unit-tests` runs unit and documentation tests; no backing service is needed.
- `make integration-tests` runs the integration binaries against the Kafka broker, which it starts through `ensure-deps`.
- `make e2e-tests` runs the Gherkin scenarios under `features/`.
- `make tests-with-coverage` runs the unit and integration suites once under instrumentation and enforces the coverage floor.
- `make mutation-tests-diff` mutates the code changed against the base ref.
- `make bats` runs the Makefile tests under `tests/CLI/bats/`.

## Rules

- Run by risk: unit tests first, then integration, then end-to-end, then coverage and mutation.
- Unit tests sit in a `tests.rs` file next to the module, never inline; shared fakes and builders live in `tests/support/`.
- Every branch of production code needs a test that fails when the branch is mutated; a surviving mutant means a missing assertion, not an exclusion.
- Never add a coverage exclusion, skip a test or weaken an assertion to obtain a pass.
- Reproduce a failure with the seeds: set `TEST_SEED` and `PROPTEST_RNG_SEED` to the values a failed run printed; the values recorded in `.cargo/config.toml` are the default.
- A test that draws seeded data binds its generator as the first statement of the test function (`let mut rng = TestRng::from_env();`) and keeps it until the test ends; helpers take `&mut TestRng` and never create one. The generator prints `TEST_SEED=<seed>` only when it is dropped while the test panics, so a generator that a helper creates and drops before the assertions run prints nothing.
- A test or scenario that writes to the broker creates a topic of its own; do not share state between tests. The service has no database, so no test prepares one.
- Debug one test at a time with `scripts/with-env.sh .env .env.test -- cargo nextest run -p <crate> <name>` after `make ensure-deps`. The wrapper sets the test environment that the Make targets set (`KAFKA_BOOTSTRAP_SERVERS` and the variables of `.env.test`); without it an integration or end-to-end test stops with a message such as `KAFKA_BOOTSTRAP_SERVERS is not set; run the tests through make`.
- Done means: the new behaviour has tests at the lowest level that can observe it, and the commands above for the touched levels pass.

## Verification

```bash
make unit-tests
make tests-with-coverage
```

Continue with `api-resource-development` for what to test per file and `verification-workflow` for the full run.
