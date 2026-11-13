---
name: quality-standards
description: Use when you need the list of gates with their thresholds and commands.
---

# Quality Standards

## Use This Skill When

- You need a gate's threshold, its Make target or the file that protects it.

## Commands

Every gate target below is a Make target, run as `make <target>`; `make ci-list` prints the ordered list that the [verification section](../../../AGENTS.md#verification) of `AGENTS.md` holds.

| Gate | Threshold | Make target | Protected file |
| --- | --- | --- | --- |
| G1 | 0 formatting differences | `format-check` | `rustfmt.toml` |
| G2 | 0 findings, warnings denied | `static-analysis` | `Cargo.toml` table `workspace.lints`, `clippy.toml` |
| G3 | cognitive complexity 8, 50 lines, 5 arguments, nesting depth 5 | `complexity` | `clippy.toml` |
| G4 | 0 violations, 0 unassigned crates | `architecture` | `architecture.toml` |
| G5 | lock file matches the manifests | `validate-manifest` | `make/gates.mk` |
| G6 | `check-config` exits 0 | `check-requirements` | `make/gates.mk` |
| G7 | 0 advisory, licence or source violations | `check-security` | `deny.toml` |
| G8 | 0 unused dependencies | `unused-dependencies` | `make/gates.mk` |
| G9 | 0 secrets findings | `secrets-scan` | `.gitleaks.toml`, `.gitleaksignore` |
| G10 | 0 HIGH or CRITICAL image findings | `image-scan` | `make/gates.mk`, `.trivyignore` |
| G11 | all unit and documentation tests pass | `unit-tests`, `doc-tests` | `.config/nextest.toml` |
| G12 | all integration tests pass | `integration-tests` | `.config/nextest.toml` |
| G13 | all scenarios pass, 0 undefined or pending steps | `e2e-tests` | `make/gates.mk` |
| G14 | 100% lines and 100% functions | `tests-with-coverage` | `make/gates.mk` |
| G15 | 0 surviving or timed-out mutants | `mutation-tests-diff` | `.cargo/mutants.toml` |
| G16 | all Bats tests pass | `bats` | `tests/CLI/bats/` |
| G17 | 0 bytes of snapshot difference | `validate-contract-snapshots` | `make/gates.mk` |
| G18 | 0 OpenAPI lint findings at any severity | `validate-openapi-spec` | `.spectral.yaml` |
| G19 | 0 unapproved OpenAPI breaking changes | `openapi-diff` | `make/gates.mk` |
| G20 | 0 unapproved GraphQL breaking changes | `graphql-diff` | `scripts/graphql-diff.sh` |
| G21 | 0 failed Schemathesis checks | `schemathesis-validate` | `make/gates.mk` |
| G22 | p99 within budget, checks above 99% | `smoke-load-tests` | `tests/Load/config.json.dist` |
| G23 | 0 repository lint findings | `lint-repo` | `.yamllint.yml`, `.markdownlint-cli2.yaml`, `.hadolint.yaml`, `_typos.toml`, `taplo.toml`, `.github/zizmor.yml`, `.prettierrc.yml`, `.prettierignore` |
| G24 | 0 load-script lint findings | `lint-load-tests` | `tests/Load/eslint.config.js` |
| G25 | 0 unapproved protected changes, 0 missing directories | `validate-configuration` | `protected-config.toml` |
| G26 | 0 documentation-drift violations | `docs-check` | `scripts/check-docs.sh` |
| G27 | valid model, its three views, its components match the workspace and a relationship for every dependency between them | `validate-architecture-model` | `scripts/validate-architecture-model.sh` |
| G28 | pull-request title matches the commit rule | CI only: needs the pull request | `scripts/check-commit-message.sh` |
| G29 | every crate builds alone, with each feature | `check-crate-isolation` | `make/gates.mk` |
| G30 | 0 differences from generated query metadata | `validate-sqlx-metadata` | `make/gates.mk` |
| G31 | 0 repository-invariant violations | `validate-invariants` | `scripts/check-invariants.sh` |

## Rules

- This table is the only place in the guidance that states a threshold; other files link here.
- A protected file changes only with the approval label named in `protected-config.toml`; never edit one to obtain a pass.
- A threshold is raised only by a recorded measurement and a labelled pull request.

## Verification

```bash
make validate-configuration
make validate-invariants
```

Continue with `verification-workflow` to run the gates and with `complexity-management` for the complexity limits.
