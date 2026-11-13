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
- A budget in `tests/Load/config.json.dist` is a protected value; raising it needs the configuration approval label and a recorded measurement in `docs/load-testing.md`.
- Never loosen a budget so that a failing run passes; find the slow path.
- Results go to `tests/Load/results/` and stay out of git.
- Cloud runs use `make aws-load-tests` and must be cleaned up with `make aws-load-tests-cleanup`; the options include `branch=<name>` and `repo_url=<https url>` (needed when `origin` is a local path, or when a detached HEAD has no branch).

## Verification

```bash
make smoke-load-tests
make lint-load-tests
```

Continue with `documentation-sync` when a scenario or budget is documented and with `quality-standards` for the protected budget values.
