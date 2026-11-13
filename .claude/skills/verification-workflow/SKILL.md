---
name: verification-workflow
description: Use when running the repository checks or fixing a failing gate before declaring a change complete.
---

# Verification Workflow

## Use This Skill When

- A change is ready and must be proven before it is called complete.
- A gate fails and the owning skill must be found.

## Commands

- `make ci` runs every gate in order and prints `CI checks successfully passed!`; it stops at the first failure.
- `make ci-list` prints the gate targets of `make ci`; the same list is the block in the [verification section](../../../AGENTS.md#verification) of `AGENTS.md`, which this skill never repeats.
- The stages can be run alone, in this order:
  - `make ci-preflight`
  - `make ci-static-analysis`
  - `make ci-tests`
  - `make ci-contracts`
  - `make ci-mutation`
  - `make ci-delivery`
  - `make ci-cli`

## Rules

- Run the narrowest stage that covers the change while iterating, and `make ci` before declaring completion.
- Fix the root cause of a failing gate; never edit a protected file to obtain a pass (see `quality-standards`).
- Find the owner of a failure by gate (the ids are those of the gate table in `quality-standards`); every gate from G1 to G31 has one:
  - repository lint (G23), configuration guard (G25) and repository invariants (G31): `documentation-sync`, `quality-standards`;
  - configuration check (G6) and query metadata (G30): `api-resource-development`;
  - secrets scan (G9), production-image build and image scan (G10): `quality-standards`; fix the finding, and accept one only as a dated entry in the protected ignore file (`.gitleaksignore`, `.trivyignore`);
  - formatting, lint and complexity (G1 to G3): `complexity-management`;
  - architecture, manifest, dependency policy and unused dependencies (G4, G5, G7, G8) and crate isolation (G29): `architecture-boundary-fixer`;
  - documentation drift (G26): `documentation-sync`;
  - model validation (G27): `architecture-model-sync`;
  - unit, integration and end-to-end tests, coverage and mutation (G11 to G15) and the Makefile tests (G16): `testing-workflow`;
  - contract snapshots, OpenAPI lint, breaking-change diffs and conformance (G17 to G21): `api-contract-development`;
  - smoke load and load-script lint (G22, G24): `load-testing`;
  - pull-request title (G28): the commit rule in [`CONTRIBUTING.md`](../../../CONTRIBUTING.md#commit-and-pull-request-title).
- A documentation-only change runs the repository lint and the drift check; say so in the close-out.
- Close-out wording: list the targets that were run with their result, the targets that were not run with the reason, and every gate that stayed red with its cause.
- Never claim a target passed that was not run.

## Verification

```bash
make ci-list
```

Pick the skill named above for the failing gate; `quality-standards` lists every gate.
