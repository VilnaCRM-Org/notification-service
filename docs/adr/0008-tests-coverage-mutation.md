# 0008. Tests, coverage and mutation

Planning decisions: AD-23 (test doubles and fault injection), AD-24 (coverage and mutation scoping), DD-09 (mutation testing cadence).

## Context

The template promises that every gate can fail: 100% line and function coverage and no surviving mutant over all production source. Adapters can only be observed against real PostgreSQL and LocalStack, and tests must be able to force a dependency to fail without stopping a container.

## Decision

Test doubles are hand-written fakes in the workspace crate `test-support`, which depends on Domain crates only. Domain crates keep the few fakes they need in their own `tests.rs` files. The end-to-end suite builds the production wiring with real adapters and replaces one health probe with a failing stub to simulate an outage. Every scenario gets its own PostgreSQL database, and a new application instance is built per request, which also shows statelessness. There are no mocking libraries and no containers started by tests.

Coverage is measured over the unit and integration suites in one run and gates 100% of lines and functions, with no ignore patterns; files named `tests.rs`, directories named `tests` and build scripts are absent from the report by construction. The end-to-end suite is not instrumented. Mutation testing covers every production source file, excluding only `main.rs`, build scripts and test files, each with its reason in the scope record. Each mutant is judged by the unit and integration tests of its own crate. A surviving or timed-out mutant fails the gate, and a mutant that does not compile is not counted.

Pull requests and the local full check run mutation on the changed code only. Pushes to `main` and a weekly schedule run the full scope, split into shards of about 150 mutants, between one and eight.

## Consequences

Mutation and integration tests need the backing services. The tool environment must not carry a target directory override or build-warning denial, or mutants become unviable and gaps go unreported. Logic must not sit in the body of an `#[async_trait]` method, where coverage and complexity tools are blind. Code that only the end-to-end suite would exercise does not exist. The exclusion records are protected, so weakening a scope needs the approval label.

Alternatives rejected: mocking libraries, fakes behind cargo features, excluding adapters from mutation, counting the end-to-end run towards coverage, a full mutation run on every pull request and a fixed shard count.
