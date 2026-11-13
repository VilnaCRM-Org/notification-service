# Documentation

Index of the documents in this directory. The PHP-to-Rust mappings are part of the references: variable names in [`environment.md`](environment.md) and target names in [`make-targets.md`](make-targets.md).

## Start here

| Document | Content |
| --- | --- |
| [Getting started](getting-started.md) | prerequisites, `make doctor`, `make bootstrap`, the first request, native mode, container mode, the development container, stopping and cleaning |
| [Creating a service](new-service.md) | package identity, the example context, `make new-context`, wiring, service-owned Make targets, secrets, variables and labels, the choice of a sync flavour |
| [Troubleshooting](troubleshooting.md) | doctor findings, Docker versions, slow first start, port clashes, stale query metadata, an interrupted Bats run, rate-limited tool installation, builds behind a TLS-intercepting proxy |

## Reference

| Document | Content |
| --- | --- |
| [Environment variables](environment.md) | every variable with default, consumer and PHP name; the placeholder policy of `.env` and `.env.test` |
| [Make targets](make-targets.md) | every target with class, needs and help text; the PHP-to-Rust target table; CI-only steps; where each gate's passing run and negative test are; targets without a Bats test |
| [Request collection](requests.http) | one request for each endpoint, in the plain-text `.http` format |
| [Architecture](architecture.md) | layers, crates and bounded contexts; request flow; error model; statelessness; deployment assumptions; link to the architecture model |
| [Decision records](adr/README.md) | the architecture decision records, in the form context, decision, consequences |
| [Extension seams](extension-seams.md) | where Redis, tracing and metrics, authentication and an outbox plug in |

## Operations and project settings

| Document | Content |
| --- | --- |
| [Operations](operations.md) | migrations and rollback, the health-probe runbook, log schema, shutdown sequence, toolchain and tool-version updates, first measurements, scan exceptions |
| [Load testing](load-testing.md) | profiles, budgets, adding a scenario, the isolated environment, the AWS runner and its costs |
| [Repository settings](repository-settings.md) | required status checks, merge settings, secrets, variables and labels, code scanning, follow-ups |

The ordered list of quality gates is in the verification section of [`AGENTS.md`](../AGENTS.md#verification). Contribution rules are in [`CONTRIBUTING.md`](../CONTRIBUTING.md).
