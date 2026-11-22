---
name: load-testing
description: Use when adding, changing or running a K6 scenario or a latency budget.
---

# Load Testing

## Use This Skill When

- A K6 scenario under `tests/Load/scripts/` is added or changed.
- A latency budget in `tests/Load/config.json.dist` is questioned or measured.
- A load profile has to be run locally or on AWS.

## Commands

- `make smoke-load-tests` runs every scenario with the smoke profile; always start here.
- `make execute-load-tests-script scenario=<name>` runs one scenario with all four profiles; set `runSmoke=false`, `runAverage=false`, `runStress=false` or `runSpike=false` (each defaults to `true`) to skip profiles.
- `make load-tests` runs every scenario with all four profiles.
- `make lint-load-tests` lints and format-checks the load-test JavaScript.

## Rules

- Smoke first, then the larger profiles; the larger profiles are not part of any pipeline.
- A scenario is a script under `tests/Load/scripts/` and an entry in `tests/Load/config.json.dist`; the shipped example is `health`.
- The load targets start their own isolated production-image environment and remove it afterwards; do not point them at a shared service.
- The four `health` budgets in `tests/Load/config.json.dist` are protected values; changing one needs the configuration approval label. Every budget change, of either scenario, records its measurements in the table `Measured values` of `docs/load-testing.md` (append rows, never overwrite them).
- Derive a budget with the rule of `docs/load-testing.md`: `min(ceiling, max(50, 3 x worst p99 rounded up to 10))`, where the worst p99 is taken over three consecutive `make load-tests BUILD_IMAGE=0` runs of the production image and any hosted-runner value; the floor is 50 ms and the ceilings stay as documented.
- Never loosen a budget so that a failing run passes; find the slow path.
- Results go to `tests/Load/results/` and stay out of git.
- Cloud runs use `make aws-load-tests` and must be cleaned up with `make aws-load-tests-cleanup`; the options include `branch=<name>` and `repo_url=<https url>` (needed when `origin` is a local path, or when a detached HEAD has no branch). `LOCAL_MODE_ENV=true` is the dry run of both: every AWS call goes to the stub CLI `tests/Load/stub-aws.sh`, which needs no credentials, no emulator and no Docker.
- Every held subscriber holds one socket of the service; the production service sets the open-file limit `nofile` to 1048576, so `Too many open files (os error 24)` under load means that limit is missing from the stack under test.
- The `health` scenario makes no network call inside the service (no probe calls a backing service since SQS was removed), so a slow health run points at the service or the machine, not at a dependency.

## Verification

```bash
make smoke-load-tests
make lint-load-tests
```

Continue with `documentation-sync` when a scenario or budget is documented and with `quality-standards` for the protected budget values.
